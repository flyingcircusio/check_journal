use super::{ErrorKind, Result, ResultExt};
use regex::bytes::Regex;
use reqwest;
use serde_yaml;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use subprocess::{ExitStatus, Popen, PopenConfig, Redirection};

#[derive(Debug, Default, Deserialize)]
struct RulesFile {
    criticalpatterns: Vec<String>,
    criticalexceptions: Vec<String>,
    warningpatterns: Vec<String>,
    warningexceptions: Vec<String>,
}

#[derive(Debug, Default)]
struct RegexSet {
    matches: Vec<Regex>,
    except: Vec<Regex>,
}

#[derive(Debug, Default)]
struct Rules {
    crit: RegexSet,
    warn: RegexSet,
}

fn compile(rules: &Vec<String>, which: &str) -> Result<Vec<Regex>> {
    rules
        .iter()
        .enumerate()
        .map(|(i, r)| Regex::new(r).chain_err(|| format!("while loading {} rule {}", which, i + 1)))
        .collect()
}

impl Rules {
    pub fn try_from<R: Read>(rdr: R) -> Result<Self> {
        let r: RulesFile =
            serde_yaml::from_reader(BufReader::new(rdr)).chain_err(|| "YAML parse error")?;
        Ok(Self {
            crit: RegexSet {
                matches: compile(&r.criticalpatterns, "critical patterns")?,
                except: compile(&r.criticalexceptions, "critical exceptions")?,
            },
            warn: RegexSet {
                matches: compile(&r.warningpatterns, "warning patterns")?,
                except: compile(&r.warningexceptions, "warning exceptions")?,
            },
        })
    }

    pub fn load(source: &str) -> Result<Self> {
        if source.contains("://") {
            Self::try_from(reqwest::get(source)
                .chain_err(|| "download error")?
                .error_for_status()
                .chain_err(|| "HTTP error")?)
        } else {
            Self::try_from(File::open(source).chain_err(|| format!("cannot open rules file '{}'", source))?)
        }
    }
}

#[derive(Debug)]
pub struct App {
    pub out: Vec<u8>,
    timeout: f32,
    lines: usize,
    bytes: usize,
    journalctl: String,
    span: String,
    rules: Rules,
}

impl App {
    fn match_rules(&self, rule: &RegexSet, line: &[u8]) -> bool {
        rule.matches.iter().any(|r| r.is_match(line)) &&
            !rule.except.iter().any(|r| r.is_match(line))
    }

    fn write(&mut self, title: &str, n: usize, display: &[u8]) {
        if display.is_empty() {
            return
        }
        // FIXME BUG: display.len() is bytes, not lines
        // should somehow count lines instead
        let trunc = if n != display.len() {
            " (truncated)"
        } else {
            ""
        };
        writeln!(self.out, "\n*** {} hits{} ***", title, trunc).ok();
        self.out.write(display).ok();
    }

    fn filter(&mut self, journal: &[u8]) -> Result<String> {
        let mut n_crit = 0;
        let mut n_warn = 0;
        let mut crit = Vec::new();
        let mut warn = Vec::new();

        for line in journal.split(|&c| c as char == '\n') {
            if line.is_empty() {
                continue;
            }
            if self.match_rules(&self.rules.crit, &line) {
                n_crit += 1;
                if n_crit < self.lines {
                    // XXX why not push?
                    crit.write(&line)?;
                    crit.write(b"\n")?;
                }
            } else if self.match_rules(&self.rules.warn, &line) {
                n_warn += 1;
                if n_warn < self.lines {
                    // XXX why not push?
                    warn.write(&line)?;
                    warn.write(b"\n")?;
                }
            }
        }

        self.write("critical", n_crit, &crit);
        self.write("warning", n_warn, &warn);

        match (n_crit, n_warn) {
            (0, 0) => Ok("no matches".into()),
            (0, w) => Err(ErrorKind::Warning(w).into()),
            (c, w) => Err(ErrorKind::Critical(c, w).into()),
        }
    }

    fn examine(&mut self, exit: ExitStatus, stdout: Vec<u8>, stderr: Vec<u8>) -> Result<String> {
        match (exit, stdout, stderr) {
            (ExitStatus::Exited(0...1), ref o, ref e) if o.is_empty() && e.is_empty() => {
                Ok("no output".into())
            }
            (ExitStatus::Exited(0...1), ref o, ref e) if e.is_empty() => self.filter(o),
            (s, o, e) => {
                writeln!(self.out, "\n*** stdout ***").ok();
                self.out.write(&o)?;
                writeln!(self.out, "\n*** stderr ***").ok();
                self.out.write(&e)?;
                Err(ErrorKind::Journal(s).into())
            }
        }
    }

    pub fn run(&mut self) -> Result<String> {
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
        self.examine(exit, stdout.unwrap(), stderr.unwrap())
    }
}

impl<'a> App {
    pub fn try_from(m: &::clap::ArgMatches<'a>) -> Result<Self> {
        Ok(Self {
            out: Vec::with_capacity(8192),
            timeout: value_t!(m, "timeout", f32)?,
            lines: value_t!(m, "lines", usize)?,
            bytes: value_t!(m, "bytes", usize)?,
            journalctl: m.value_of("journalctl").expect("default_value gone").into(),
            span: m.value_of("span").expect("default_value gone").into(),
            rules: Rules::load(m.value_of("rules").unwrap())?,
        })
    }
}
