//! Check execution and reporting

use super::Opt;
use crate::rules::Rules;

use anyhow::{bail, Context, Result};
use std::fmt::Write;
use std::fs::File;
use std::process::{Command, Output, Stdio};
use std::str;

/// Return status according to Nagios guidelines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Success with general message
    Ok(String),
    /// Line counts of messages matching warning patterns
    Warning(usize),
    /// Line counts of messages matching critical and warning patterns
    Critical(usize, usize),
}

impl Default for Status {
    fn default() -> Self {
        Status::Ok(String::default())
    }
}

/// Overall status and collection of messages which match rule patterns.
#[derive(Debug, Default)]
pub struct Outcome {
    pub status: Status,
    pub message: String,
}

/// Log lines grouped into critcal and warning after applying rule sets
#[derive(Debug)]
pub struct Collection<'a> {
    rules: &'a Rules,
    critical: Vec<&'a str>,
    warning: Vec<&'a str>,
}

impl<'a> Collection<'a> {
    fn new(rules: &'a Rules) -> Self {
        Self {
            rules,
            critical: Vec::with_capacity(100),
            warning: Vec::with_capacity(100),
        }
    }

    fn push(&mut self, line: &'a str) {
        if line.is_empty() || line.starts_with("-- Logs begin ") {
            return;
        }
        if self.rules.crit.is_match(line) {
            self.critical.push(line);
        } else if self.rules.warn.is_match(line) {
            self.warning.push(line);
        }
    }
}

/// Main data structure which controls check execution. Contains program options and rule sets.
#[derive(Debug, Default)]
pub struct Check {
    opt: Opt,
    rules: Rules,
}

impl Check {
    /// Creates instance from program options. Loads specified rules file.
    pub fn new(opt: super::Opt) -> Result<Self> {
        let rules = Rules::load(&opt.rules_yaml)?;
        Ok(Self { opt, rules })
    }

    /// Runs journalctl. Optionally re-runs journalctl if state file contains garbage.
    pub fn exec_journalctl(&self) -> Result<Output> {
        let mut cmd = Command::new(&self.opt.journalctl);
        cmd.arg("--no-pager")
            .arg(&format!("--since=-{}", self.opt.span))
            .stdin(Stdio::null());
        if let Some(units) = &self.opt.unit {
            cmd.args(
                units
                    .iter()
                    .map(|u| format!("--unit={}", u))
                    .collect::<Vec<_>>(),
            );
        if self.opt.user {
            cmd.arg("--user");
        }
        if let Some(sf) = &self.opt.statefile {
            cmd.arg(&format!("--cursor-file={}", sf.display()));
        }
        let mut out = cmd.output();
        match (&self.opt.statefile, &out) {
            (Some(sf), Ok(res))
                if String::from_utf8_lossy(&res.stderr).contains("Failed to seek to cursor") =>
            {
                // This is probably caused by on old-style (pre-1.1.2) status file.
                // Truncate the status file and try again.
                out = File::create(sf).and_then(|_| cmd.output());
            }
            _ => (),
        }
        let out =
            out.with_context(|| format!("Failed to execute {}", self.opt.journalctl.display()))?;
        let code = out.status.code().unwrap_or(-1);
        if code != 0 {
            bail!(
                "journalctl error: {} (exit {})",
                String::from_utf8_lossy(&out.stderr).trim().to_owned(),
                code
            )
        } else {
            Ok(out)
        }
    }

    fn format_message(&self, title: &str, matches: &'_ [&'_ str]) -> String {
        let mut msg = String::with_capacity(4096);
        if matches.is_empty() {
            return msg;
        }
        let max_lines = match (self.opt.no_limit, self.opt.limit) {
            (true, _) => usize::MAX,
            (false, 0) => usize::MAX,
            (false, l) => l,
        };
        let trunc = match matches.len() {
            n if n > max_lines => " (truncated)",
            _ => "",
        };
        writeln!(msg, "*** {}{} ***\n", title, trunc).ok();
        for m in matches.iter().take(max_lines) {
            writeln!(msg, "{}", m).ok();
        }
        msg
    }

    /// Evaluates journalctl output and returrns appropriate result
    pub fn evaluate(&mut self, journal: Output) -> Result<Outcome> {
        let mut collection = Collection::new(&self.rules);
        let stdout = String::from_utf8_lossy(&journal.stdout);
        for line in stdout.split('\n') {
            collection.push(line)
        }
        let mut msg = Vec::with_capacity(2);
        if !collection.critical.is_empty() {
            msg.push(self.format_message("CRITICAL MATCHES", &collection.critical))
        }
        if !collection.warning.is_empty() {
            msg.push(self.format_message("WARNING MATCHES", &collection.warning))
        }
        Ok(Outcome {
            status: match (collection.critical.len(), collection.warning.len()) {
                (c, w) if c > 0 => Status::Critical(c, w),
                (0, w) if w > 0 => Status::Warning(w),
                (_, _) => Status::Ok("No matches".into()),
            },
            message: msg.join("\n"),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tests::{fixture, RULES};
    use std::fs;
    use std::io::Write;

    #[test]
    fn push_to_collection() {
        let mut c = Collection::new(&RULES);
        assert!(c.critical.is_empty());
        assert!(c.warning.is_empty());
        c.push(""); // should be ignored
        c.push("-- Logs begin at Mon 2020-10-19 06:28:37 CEST"); // should be ignored
        c.push("warning: 1");
        c.push("error: 2");
        assert_eq!(&c.warning, &["warning: 1"]);
        assert_eq!(&c.critical, &["error: 2"]);
    }

    fn check(journalctl_fixture: &str) -> Check {
        Check {
            opt: Opt {
                journalctl: fixture(journalctl_fixture),
                ..Opt::default()
            },
            rules: RULES.clone(),
        }
    }

    #[test]
    fn run_journalctl() {
        let check = check("journalctl-cursor-file.sh");
        assert_eq!(
            check
                .exec_journalctl()
                .expect("exec_journalctl() failed")
                .stdout,
            fs::read(fixture("journal.txt")).unwrap()
        );
    }

    #[test]
    fn run_journalctl_twice_with_corrupted_state_file() {
        let mut tf = tempfile::NamedTempFile::new().unwrap();
        tf.write_all(b"garbage\n").unwrap();
        tf.flush().ok();
        let mut check = check("journalctl-cursor-file.sh");
        check.opt.statefile = Some(tf.path().into());
        check.exec_journalctl().unwrap();
        assert_eq!(std::fs::read_to_string(tf.path()).unwrap(), "new-format\n");
    }

    #[test]
    fn handle_journalctl_failure() {
        let check = check("journalctl-error.sh");
        assert_eq!(
            check.exec_journalctl().unwrap_err().to_string(),
            "journalctl error: dummy for testing (exit 1)"
        );
    }

    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    #[test]
    fn evaluate_ok() {
        let mut check = check("journalctl-ok.sh");
        let o = check
            .evaluate(Output {
                status: ExitStatus::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
            .unwrap();
        assert_eq!(o.status, Status::Ok("No matches".into()));
    }

    #[test]
    fn evaluate_warning() {
        let mut check = check("journalctl-ok.sh");
        let o = check
            .evaluate(Output {
                status: ExitStatus::from_raw(0),
                stdout: "WARN 1\nWARN 2\n".as_bytes().into(),
                stderr: Vec::new(),
            })
            .unwrap();
        assert_eq!(o.status, Status::Warning(2));
        assert_eq!(
            o.message,
            String::from(
                "\
            *** WARNING MATCHES ***\n\
            \n\
            WARN 1\n\
            WARN 2\n"
            )
        )
    }

    #[test]
    fn evaluate_critical() {
        let mut check = check("journalctl-ok.sh");
        let o = check
            .evaluate(Output {
                status: ExitStatus::from_raw(0),
                stdout: "error: 1\nerror: 2\nwarning: 1\n".as_bytes().into(),
                stderr: Vec::new(),
            })
            .unwrap();
        assert_eq!(o.status, Status::Critical(2, 1));
        assert_eq!(
            o.message,
            String::from(
                "\
            *** CRITICAL MATCHES ***\n\
            \n\
            error: 1\n\
            error: 2\n\
            \n\
            *** WARNING MATCHES ***\n\
            \n\
            warning: 1\n"
            )
        )
    }

    #[test]
    fn report_limit() {
        let mut check = check("journalctl-ok.sh");
        check.opt.limit = 1;
        let o = check
            .evaluate(Output {
                status: ExitStatus::from_raw(0),
                stdout: "WARN 1\nWARN 2\n".as_bytes().into(),
                stderr: Vec::new(),
            })
            .unwrap();
        assert_eq!(o.status, Status::Warning(2));
        assert_eq!(
            o.message,
            String::from(
                "\
            *** WARNING MATCHES (truncated) ***\n\
            \n\
            WARN 1\n"
            )
        )
    }
}
