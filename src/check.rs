use rules::Rules;
use std::io::Write;
use subprocess::ExitStatus::Exited;
use subprocess::{ExitStatus, Popen, PopenConfig, Redirection};
use {ErrorKind, Result, ResultExt};

#[derive(Debug, Clone, Default)]
struct Filtered<'a> {
    crit: Vec<&'a [u8]>,
    warn: Vec<&'a [u8]>,
}

impl<'a> Filtered<'a> {
    fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
pub struct Check {
    timeout: u32,
    lines: usize,
    bytes: usize,
    journalctl: String,
    span: String,
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
        let trunc = if matches.len() > self.lines {
            " (truncated)"
        } else {
            ""
        };
        writeln!(out, "\n*** {} hits{} ***\n", title, trunc).ok();
        matches.iter().take(self.lines).for_each(|m| {
            out.extend_from_slice(m);
            out.push(b'\n');
        })
    }

    fn report(&self, out: &mut Vec<u8>, journal_lines: &[u8]) -> Result<String> {
        let res = self.filter(journal_lines);
        self.fmt_matches(out, "Critical", &res.crit);
        self.fmt_matches(out, "Warning", &res.warn);

        if self.bytes > 0 && out.len() > self.bytes {
            out.truncate(self.bytes);
            out.extend_from_slice(b"[...]\n")
        }

        match (res.crit.len(), res.warn.len()) {
            (0, 0) => Ok("no matches".into()),
            (0, w) => Err(ErrorKind::Warning(w).into()),
            (c, w) => Err(ErrorKind::Critical(c, w).into()),
        }
    }

    fn examine(
        &self,
        out: &mut Vec<u8>,
        exit: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    ) -> Result<String> {
        match (exit, stdout, stderr) {
            (Exited(0...1), ref o, ref e) if o.is_empty() && e.is_empty() => Ok("no output".into()),
            (Exited(0...1), ref o, ref e) if e.is_empty() => self.report(out, o),
            (s, o, e) => {
                writeln!(out, "\n*** stdout ***")?;
                out.write_all(&o)?;
                writeln!(out, "\n*** stderr ***")?;
                out.write_all(&e)?;
                Err(ErrorKind::Journal(s).into())
            }
        }
    }

    pub fn run(&mut self, out: &mut Vec<u8>) -> Result<String> {
        if self.timeout > 0 {
            ::timeout::install(self.timeout)?;
        }
        let since = format!("--since=-{}", self.span);
        // compromise between inaccurate counts and memory usage cap
        let lines = format!("--lines={}", 10 * self.lines);
        let cmdline = &[&self.journalctl, "--no-pager", &since, &lines][..];
        let mut p = Popen::create(
            cmdline,
            PopenConfig {
                stdin: Redirection::Pipe,
                stdout: Redirection::Pipe,
                stderr: Redirection::Pipe,
                ..Default::default()
            },
        ).chain_err(|| format!("failed to execute '{}'", self.journalctl))?;
        let (stdout, stderr) = p.communicate_bytes(Some(b""))?;
        let exit = p.wait()?;
        // stdout/stderr is always some value b/o Redirection::Pipe
        self.examine(out, exit, stdout.unwrap(), stderr.unwrap())
    }
}

impl<'a> Check {
    pub fn try_from(m: &::clap::ArgMatches<'a>) -> Result<Self> {
        Ok(Self {
            timeout: value_t!(m, "timeout", u32)?,
            lines: value_t!(m, "lines", usize)?,
            bytes: value_t!(m, "bytes", usize)?,
            journalctl: m.value_of("journalctl")
                .expect("missing default_value")
                .into(),
            span: m.value_of("span").expect("missing default_value").into(),
            rules: Rules::load(m.value_of("RULES").expect("missing option"))?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tests::FIXTURES;
    use Error;

    fn stringify<'a>(res: Vec<&[u8]>) -> Vec<String> {
        res.iter()
            .map(|s| String::from_utf8_lossy(*s).into_owned())
            .collect()
    }

    /// `Check` instance with sensible defaults
    fn check_fac() -> Check {
        let mut c = Check::default();
        c.rules = Rules::load(FIXTURES.join("rules.yaml")).expect("load from file");
        c.lines = 10;
        c.bytes = 4096;
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
        c.lines = 1;
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
            c.report(&mut out, b"all fine").unwrap(),
            "no matches".to_owned()
        );
        match c.report(&mut out, b"error").unwrap_err() {
            Error(ErrorKind::Critical(n, m), _) => assert_eq!((n, m), (1, 0)),
            e => panic!("unexpected error {:?}", e),
        }
        match c.report(&mut out, b"warning").unwrap_err() {
            Error(ErrorKind::Warning(n), _) => assert_eq!(n, 1),
            e => panic!("unexpected error {:?}", e),
        }
        assert_eq!(
            String::from_utf8_lossy(&out),
            "\n*** Critical hits ***\n\nerror\n\n*** Warning hits ***\n\nwarning\n"
        );
    }

    #[test]
    fn report_should_truncate_output() {
        let mut c = check_fac();
        c.bytes = 50;
        let mut out = Vec::new();
        c.report(&mut out, b"error line\nwarning line\n")
            .unwrap_err();
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
            "no output"
        );
    }

    #[test]
    fn should_match_on_exit_status_0_or_1() {
        let c = check_fac();
        assert_eq!(
            c.examine(&mut vec![], Exited(0), b"log line".to_vec(), vec![])
                .unwrap(),
            "no matches"
        );
        assert_eq!(
            c.examine(&mut vec![], Exited(1), b"log line".to_vec(), vec![])
                .unwrap(),
            "no matches"
        );
    }

    #[test]
    fn should_fail_on_stderr_and_exit_status_0() {
        let c = check_fac();
        match c.examine(&mut vec![], Exited(0), vec![], b"error output".to_vec())
            .unwrap_err()
        {
            Error(ErrorKind::Journal(Exited(0)), _) => (),
            e => panic!("unexpected error {}", e),
        }
    }

    #[test]
    fn should_fail_on_exit_status() {
        let c = check_fac();
        match c.examine(&mut vec![], Exited(3), vec![], vec![])
            .unwrap_err()
        {
            Error(ErrorKind::Journal(Exited(3)), _) => (),
            e => panic!("unexpected error {}", e),
        }
    }
}
