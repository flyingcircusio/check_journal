use super::{ErrorKind, Result, ResultExt};
use rules::Rules;
use std::io::Write;
use subprocess::{ExitStatus, Popen, PopenConfig, Redirection};

#[derive(Debug)]
pub struct Check {
    timeout: f32,
    lines: usize,
    bytes: usize,
    journalctl: String,
    span: String,
    rules: Rules,
}

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

    fn report(&mut self, out: &mut Vec<u8>, journal_lines: &[u8]) -> Result<String> {
        let res = self.filter(journal_lines);
        self.fmt_matches(out, "Critical", &res.crit);
        self.fmt_matches(out, "Warning", &res.warn);

        match (res.crit.len(), res.warn.len()) {
            (0, 0) => Ok("no matches".into()),
            (0, w) => Err(ErrorKind::Warning(w).into()),
            (c, w) => Err(ErrorKind::Critical(c, w).into()),
        }
    }

    fn examine(
        &mut self,
        out: &mut Vec<u8>,
        exit: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    ) -> Result<String> {
        match (exit, stdout, stderr) {
            (ExitStatus::Exited(0...1), ref o, ref e) if o.is_empty() && e.is_empty() => {
                Ok("no output".into())
            }
            (ExitStatus::Exited(0...1), ref o, ref e) if e.is_empty() => self.report(out, o),
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
        let since = format!("--since=-{}", self.span);
        let mut p = Popen::create(
            &[&self.journalctl, "--no-pager", &since][..],
            PopenConfig {
                stdin: Redirection::None,
                stdout: Redirection::Pipe,
                stderr: Redirection::Pipe,
                ..Default::default()
            },
        ).chain_err(|| format!("failed to execute '{}'", self.journalctl))?;
        let exit = p.wait()?;
        let (stdout, stderr) = p.communicate_bytes(None)?;
        // stdout/stderr is always some value b/o PopenConfig
        self.examine(out, exit, stdout.unwrap(), stderr.unwrap())
    }
}

impl<'a> Check {
    pub fn try_from(m: &::clap::ArgMatches<'a>) -> Result<Self> {
        Ok(Self {
            timeout: value_t!(m, "timeout", f32)?,
            lines: value_t!(m, "lines", usize)?,
            bytes: value_t!(m, "bytes", usize)?,
            journalctl: m.value_of("journalctl").expect("default_value gone").into(),
            span: m.value_of("span").expect("default_value gone").into(),
            rules: Rules::load(m.value_of("rules").unwrap())?,
        })
    }
}
