//! Check execution and reporting

use super::Opt;
use crate::rules::Rules;

use anyhow::{anyhow, Error, Result};
use std::fs::File;
use std::io::Write;
use std::str;
use subprocess::ExitStatus::Exited;
use subprocess::{Exec, ExitStatus, Redirection};

/// Log lines grouped into critcal, warning and special after applying rule sets
#[derive(Debug, Clone, Default)]
struct Filtered<'a> {
    crit: Vec<&'a [u8]>,
    warn: Vec<&'a [u8]>,
}

impl<'a> Filtered<'a> {
    fn collect(journal: &'a [u8], rules: &'_ Rules) -> Filtered<'a> {
        journal
            .split(|&c| c as char == '\n')
            .filter(|l| !l.is_empty() && !l.starts_with(b"-- Logs begin "))
            .fold(Default::default(), |mut acc, line| {
                if rules.crit.is_match(line) {
                    acc.crit.push(line);
                } else if rules.warn.is_match(line) {
                    acc.warn.push(line);
                };
                acc
            })
    }
}

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

/// Overall status and collection of messages which match rule patterns.
#[derive(Debug)]
pub struct Outcome {
    pub status: Result<Status>,
    pub message: Vec<u8>,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            status: Ok(Status::Ok(String::new())),
            message: vec![],
        }
    }
}

impl Outcome {
    fn push(mut self, title: &str, matches: &[&[u8]], max_lines: usize) -> Self {
        if matches.is_empty() {
            return self;
        }
        let trunc = match matches.len() {
            n if n > max_lines => " (truncated)",
            _ => "",
        };
        writeln!(self.message, "\n*** {} hits{} ***\n", title, trunc).ok();
        for m in matches.iter().take(max_lines) {
            self.message.extend_from_slice(m);
            self.message.push(b'\n');
        }
        self
    }

    fn matched(journal: &[u8], rules: &Rules, max_lines: usize) -> Self {
        let mut res = Self::default();
        let filt = Filtered::collect(journal, rules);
        res = res.push("critical", &filt.crit, max_lines);
        res = res.push("warning", &filt.warn, max_lines);
        res.status = Ok(match (filt.crit.len(), filt.warn.len()) {
            (0, 0) => Status::Ok("no matches".to_owned()),
            (0, w) => Status::Warning(w),
            (c, w) => Status::Critical(c, w),
        });
        res
    }

    fn empty() -> Self {
        Self {
            status: Ok(Status::Ok("no output".to_owned())),
            ..Default::default()
        }
    }

    fn failed(exit: ExitStatus, stdout: &[u8], stderr: &[u8]) -> Self {
        let mut msg = vec![];
        writeln!(msg, "\n*** stdout ***").ok();
        msg.write_all(stdout).ok();
        writeln!(msg, "\n*** stderr ***").ok();
        msg.write_all(stderr).ok();
        Self {
            status: Err(anyhow!("journalctl failed with {:?}", exit)),
            message: msg,
        }
    }

    fn error(e: Error) -> Self {
        Outcome {
            status: Err(e),
            ..Self::default()
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

    fn examine(&self, exit: ExitStatus, stdout: &[u8], stderr: &[u8]) -> Outcome {
        match (exit, stdout, stderr) {
            (Exited(0..=1), o, e) if o.is_empty() && e.is_empty() => Outcome::empty(),
            (Exited(0..=1), o, _) if !o.is_empty() => {
                Outcome::matched(o, &self.rules, self.opt.lines)
            }
            (x, o, e) => Outcome::failed(x, o, e),
        }
    }

    /// Executes journalctl and evaluates results.
    pub fn run(&mut self) -> Outcome {
        let mut cmd = Exec::cmd(&self.opt.journalctl)
            .args(&["--no-pager"])
            // 10x lines is a compromise between inaccurate counts and memory usage cap
            .arg(&format!("--lines={}", 10 * self.opt.lines))
            .arg(&format!("--since=-{}", self.opt.span))
            .stdout(Redirection::Pipe)
            .stderr(Redirection::Pipe);
        if let Some(sf) = &self.opt.statefile {
            cmd = cmd.arg(&format!("--cursor-file={}", sf.display()));
        }
        let mut cap = cmd.clone().capture();
        match (&self.opt.statefile, &cap) {
            (Some(sf), Ok(res)) if res.stderr_str().contains("Failed to seek to cursor") => {
                // This is probably caused by on old-style (pre-1.1.2) status file.
                // Truncate the status file and try again.
                cap = File::create(sf)
                    .map_err(|e| e.into())
                    .and_then(|_| cmd.capture());
            }
            _ => (),
        }
        cap.map(|c| self.examine(c.exit_status, &c.stdout, &c.stderr))
            .unwrap_or_else(|e| {
                Outcome::error(anyhow!(
                    "Failed to execute '{}': {}",
                    self.opt.journalctl,
                    e
                ))
            })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn stringify<'a>(res: Vec<&[u8]>) -> Vec<String> {
        res.iter()
            .map(|s| String::from_utf8_lossy(*s).into_owned())
            .collect()
    }

    /// `Check` instance with sensible defaults
    fn check_fac() -> Check {
        let mut c = Check::default();
        c.rules = Rules::load(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/rules.yaml"))
            .expect("load from file");
        c.opt.lines = 10;
        c
    }

    #[test]
    fn filter_crit_warn() {
        let j = include_bytes!("../fixtures/journal.txt");
        let rules = Rules::load(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/rules.yaml"))
            .expect("load rules");
        let res = Filtered::collect(&j[..], &rules);
        assert_eq!(
            stringify(res.crit),
            vec![
                "Mai 31 16:42:47 session[14529]: aborting",
                "Mai 31 16:42:50 program[14133]: *** CRITICAL ERROR",
                "Mai 31 16:43:20 program1[6094]: timestamp:\"1527780630\",level:\"abort\"",
            ]
        );
        assert_eq!(
            stringify(res.warn),
            vec![
                "Mai 31 16:42:47 session[14529]: assertion '!window->override_redirect' failed",
                "Mai 31 16:42:49 user[14529]: 0 errors, 1 failures",
            ]
        );
    }

    #[test]
    fn outcome_should_list_matches() {
        let out = Outcome::default().push("test1", &[b"first match", b"second match"], 10);
        assert_eq!(
            String::from_utf8_lossy(&out.message),
            "\n*** test1 hits ***\n\
             \n\
             first match\n\
             second match\n"
        );
    }

    #[test]
    fn outcome_should_truncate_lines() {
        let out = Outcome::default().push("test2", &[b"first match", b"second match"], 1);
        assert_eq!(
            String::from_utf8_lossy(&out.message),
            "\n*** test2 hits (truncated) ***\n\
             \n\
             first match\n"
        );
    }

    #[test]
    fn should_return_warn_crit_status() {
        let r = Rules::load(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/rules.yaml"))
            .expect("load rules");
        let out = Outcome::matched(b"all fine", &r, 10);
        assert_eq!(out.status.unwrap(), Status::Ok("no matches".to_owned()));
        let out = Outcome::matched(b"error", &r, 10);
        assert_eq!(out.status.unwrap(), Status::Critical(1, 0));
        let out = Outcome::matched(b"warning", &r, 10);
        assert_eq!(out.status.unwrap(), Status::Warning(1));
    }

    #[test]
    fn should_report_no_output() {
        assert_eq!(
            Outcome::empty().status.unwrap(),
            Status::Ok("no output".to_owned())
        );
    }

    #[test]
    fn should_match_on_exit_0_or_1() {
        let c = check_fac();
        for code in 0..=1 {
            assert_eq!(
                c.examine(Exited(code), b"log line", b"").status.unwrap(),
                Status::Ok("no matches".to_owned())
            );
        }
    }

    #[test]
    fn should_disregard_stderr_on_exit_0_1() {
        let c = check_fac();
        for code in 0..=1 {
            assert_eq!(
                c.examine(Exited(code), b"log", b"strange debug msg")
                    .status
                    .unwrap(),
                Status::Ok("no matches".to_owned())
            );
        }
    }

    #[test]
    fn should_fail_on_exit_status() {
        let c = check_fac();
        assert_eq!(
            c.examine(Exited(3), b"", b"")
                .status
                .unwrap_err()
                .to_string(),
            "journalctl failed with Exited(3)"
        );
    }

    #[test]
    fn should_ignore_first_line() {
        let c = check_fac();
        assert_eq!(
            c.examine(Exited(0), b"-- Logs begin with error", b"")
                .status
                .unwrap(),
            Status::Ok("no matches".to_owned())
        );
    }
}
