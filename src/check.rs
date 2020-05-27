use super::{Opt, Status};
use crate::rules::Rules;

use anyhow::{anyhow, Context, Result};
use std::io::Write;
use subprocess::ExitStatus::Exited;
use subprocess::{Exec, ExitStatus, Redirection::*};

#[derive(Debug, Clone, Default)]
struct Filtered<'a> {
    crit: Vec<&'a [u8]>,
    warn: Vec<&'a [u8]>,
}

impl Filtered<'_> {
    fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
pub struct Check {
    opt: Opt,
    rules: Rules,
}

impl Check {
    fn filter<'a>(&self, journal: &'a [u8]) -> Filtered<'a> {
        journal
            .split(|&c| c as char == '\n')
            .filter(|l| !l.is_empty())
            .fold(Filtered::new(), |mut acc, line| {
                if self.rules.crit.is_match(line) {
                    acc.crit.push(line);
                } else if self.rules.warn.is_match(line) {
                    acc.warn.push(line);
                };
                acc
            })
    }

    fn fmt_matches(&self, out: &mut Vec<u8>, title: &str, matches: &[&[u8]]) {
        if matches.is_empty() {
            return;
        }
        let trunc = if matches.len() > self.opt.lines {
            " (truncated)"
        } else {
            ""
        };
        writeln!(out, "\n*** {} hits{} ***\n", title, trunc).ok();
        matches.iter().take(self.opt.lines).for_each(|m| {
            out.extend_from_slice(m);
            out.push(b'\n');
        })
    }

    fn report(&self, out: &mut Vec<u8>, journal_lines: &[u8]) -> Status {
        let res = self.filter(journal_lines);
        self.fmt_matches(out, "Critical", &res.crit);
        self.fmt_matches(out, "Warning", &res.warn);

        if self.opt.bytes > 0 && out.len() > self.opt.bytes {
            out.truncate(self.opt.bytes);
            out.extend_from_slice(b"[...]\n")
        }

        match (res.crit.len(), res.warn.len()) {
            (0, 0) => Status::Ok("no matches".to_owned()),
            (0, w) => Status::Warning(w),
            (c, w) => Status::Critical(c, w),
        }
    }

    fn examine(
        &self,
        out: &mut Vec<u8>,
        exit: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    ) -> Result<Status> {
        match (exit, stdout, stderr) {
            (Exited(0..=1), ref o, ref e) if o.is_empty() && e.is_empty() => {
                Ok(Status::Ok("no output".to_owned()))
            }
            (Exited(0..=1), ref o, _) if !o.is_empty() => Ok(self.report(out, o)),
            (s, o, e) => {
                writeln!(out, "\n*** stdout ***")?;
                out.write_all(&o)?;
                writeln!(out, "\n*** stderr ***")?;
                out.write_all(&e)?;
                Err(anyhow!("journalctl failed with {:?}", s))
            }
        }
    }

    pub fn run(&mut self, out: &mut Vec<u8>) -> Result<Status> {
        let since = format!("--since=-{}", self.opt.span);
        // compromise between inaccurate counts and memory usage cap
        let lines = format!("--lines={}", 10 * self.opt.lines);
        let c = Exec::cmd(&self.opt.journalctl)
            .args(&["--no-pager", &since, &lines])
            .stdout(Pipe)
            .stderr(Pipe)
            .capture()
            .with_context(|| format!("failed to execute '{}'", self.opt.journalctl))?;
        self.examine(out, c.exit_status, c.stdout, c.stderr)
    }

    pub fn new(opt: super::Opt) -> Result<Self> {
        let rules = Rules::load(&opt.rules_yaml)?;
        Ok(Self { opt, rules })
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
        c.opt.bytes = 4096;
        c
    }

    #[test]
    fn filter_crit_warn() {
        let c = check_fac();
        let j = include_bytes!("../fixtures/journal.txt");
        let res = c.filter(&j[..]);
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
    fn fmt_matches_should_list_matches() {
        let c = check_fac();
        let mut out = Vec::new();
        c.fmt_matches(&mut out, "test1", &[b"first match", b"second match"]);
        assert_eq!(
            String::from_utf8_lossy(&out),
            "\n*** test1 hits ***\n\
             \n\
             first match\n\
             second match\n"
        );
    }

    #[test]
    fn fmt_matches_should_truncate_lines() {
        let mut c = check_fac();
        c.opt.lines = 1;
        let mut out = Vec::new();
        c.fmt_matches(&mut out, "test2", &[b"first match", b"second match"]);
        assert_eq!(
            String::from_utf8_lossy(&out),
            "\n*** test2 hits (truncated) ***\n\
             \n\
             first match\n"
        );
    }

    #[test]
    fn report_should_return_warn_crit_status() {
        let c = check_fac();
        let mut out = Vec::new();
        assert_eq!(
            c.report(&mut out, b"all fine"),
            Status::Ok("no matches".to_owned())
        );
        match c.report(&mut out, b"error") {
            Status::Critical(n, m) => assert_eq!((n, m), (1, 0)),
            e => panic!("unexpected status {:?}", e),
        }
        match c.report(&mut out, b"warning") {
            Status::Warning(n) => assert_eq!(n, 1),
            e => panic!("unexpected status {:?}", e),
        }
        assert_eq!(
            String::from_utf8_lossy(&out),
            "\n*** Critical hits ***\n\nerror\n\n*** Warning hits ***\n\nwarning\n"
        );
    }

    #[test]
    fn report_should_truncate_output() {
        let mut c = check_fac();
        c.opt.bytes = 50;
        let mut out = Vec::new();
        c.report(&mut out, b"error line\nwarning line\n");
        assert_eq!(
            String::from_utf8_lossy(&out),
            "\n*** Critical hits ***\n\nerror line\n\n*** Warning hi[...]\n"
        )
    }

    #[test]
    fn should_report_no_output() {
        let c = check_fac();
        assert_eq!(
            c.examine(&mut vec![], Exited(0), vec![], vec![]).unwrap(),
            Status::Ok("no output".to_owned())
        );
    }

    #[test]
    fn should_match_on_exit_0_or_1() {
        let c = check_fac();
        for code in 0..=1 {
            assert_eq!(
                c.examine(&mut vec![], Exited(code), b"log line".to_vec(), vec![])
                    .unwrap(),
                Status::Ok("no matches".to_owned())
            );
        }
    }

    #[test]
    fn should_disregard_stderr_on_exit_0_1() {
        let c = check_fac();
        for code in 0..=1 {
            assert_eq!(
                c.examine(
                    &mut vec![],
                    Exited(code),
                    b"log line".to_vec(),
                    b"strange debug msg".to_vec()
                )
                .unwrap(),
                Status::Ok("no matches".to_owned())
            );
        }
    }

    #[test]
    fn should_fail_on_exit_status() {
        let c = check_fac();
        assert_eq!(
            c.examine(&mut vec![], Exited(3), vec![], vec![])
                .unwrap_err()
                .to_string(),
            "journalctl failed with Exited(3)"
        );
    }
}
