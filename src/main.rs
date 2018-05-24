#[macro_use] extern crate clap;
#[macro_use] extern crate derive_error_chain;
extern crate error_chain;
extern crate subprocess;
extern crate regex;

use clap::Arg;
use std::io::{Write, stdout};
use std::process;
use subprocess::{Popen, PopenConfig, Redirection, ExitStatus};
use regex::Regex;

#[derive(Debug, ErrorChain)]
enum ErrorKind {
    #[error_chain(foreign)]
    Fmt(std::fmt::Error),

    #[error_chain(foreign)]
    Io(std::io::Error),

    #[error_chain(foreign)]
    Clap(clap::Error),

    #[error_chain(foreign)]
    Popen(subprocess::PopenError),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_, _| "critical check result""#)]
    #[error_chain(display = r#"|crit, warn|
                  write!(f, "{} critical lines, {} warning lines found", crit, warn)"#)]
    Critical(i64, i64),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "warning check result""#)]
    #[error_chain(display = r#"|warn| write!(f, "{} warning lines found", warn)"#)]
    Warning(i64),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "unexpected journalctl behaviour""#)]
    #[error_chain(display = r#"|s| write!(f, "journalctl failed with exit code {:?}", s)"#)]
    Journal(ExitStatus),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "regex error""#)]
    #[error_chain(display = r#"|re| write!(f, "regex parse failure: {}", re)"#)]
    Regex(i32),
}

#[derive(Debug, Default)]
struct Rules {
    crit: RegexSet,
    warn: RegexSet
}

#[derive(Debug, Default)]
struct RegexSet {
    matches: Vec<Regex>,
    except: Vec<Regex>
}

#[derive(Debug)]
struct App {
    timeout: f32,
    lines: u32,
    bytes: u64,
    journalctl: String,
    span: String,
    rules: Rules,
    out: Vec<u8>,
}

impl App {
    fn run(&mut self) -> Result<String> {
        let since = format!("--since=-{}", self.span);
        let mut p = Popen::create(&[&self.journalctl, "--no-pager", &since][..],
                                  PopenConfig { stdin: Redirection::None, stdout: Redirection::Pipe, stderr: Redirection::Pipe, ..Default::default() })
            // XXX
            .chain_err(|| "failed to execute {}")?;
        let exit = p.wait()?;
        let (stdout, stderr) = p.communicate_bytes(None)?;
        let (stdout, stderr) = (stdout.unwrap(), stderr.unwrap());
        match (exit, stdout, stderr) {
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

    fn filter(&mut self, journal: &[u8]) -> Result<String> {
        // debug Ok(String::from_utf8_lossy(journal).to_string())
        Ok("no entries".into())
    }
}

impl<'a> App {
    fn try_from(m: &clap::ArgMatches<'a>) -> Result<Self> {
        Ok(Self {
            timeout: value_t!(m, "timeout", f32)?,
            lines: value_t!(m, "lines", u32)?,
            bytes: value_t!(m, "bytes", u64)?,
            journalctl: m.value_of("journalctl").expect("default_value gone").into(),
            span: m.value_of("span").expect("default_value gone").into(),
            rules: Rules::default(),
            out: Vec::with_capacity(8192)
        })
    }
}

fn output(res: Result<String>)-> i32 {
    let (keyword, summary, exitcode) = match res {
        Ok(summary) => ("OK", summary, 0),
        Err(Error(e @ ErrorKind::Warning(_), _)) => ("WARNING", e.description().to_owned(), 1),
        Err(Error(e @ ErrorKind::Critical(_, _), _)) => ("CRITICAL", e.description().to_owned(), 2),
        Err(Error(e, _)) => ("UNKNOWN", e.description().to_owned(), 3),
    };
    println!("{} {} - {}", crate_name!(), keyword, summary);
    exitcode
}

fn main() {
    let matches = app_from_crate!()
        .arg(Arg::with_name("rules").value_name("RULES").required(true)
             .help("YAML logcheck rules, local file name or URL"))
        .arg(Arg::with_name("journalctl").long("journalctl").default_value("journalctl")
             .help("Path to journalctl executable"))
        .arg(Arg::with_name("timeout").short("t").long("timeout").value_name("T")
             .default_value("60").help("Aborts check execution after T seconds"))
        .arg(Arg::with_name("span").short("s").long("span").value_name("EXPR")
             .default_value("600")
             .help("Journal search time span (seconds; time suffixes accepted)"))
        .arg(Arg::with_name("lines").short("l").long("limit-lines").value_name("N")
             .default_value("100") .help("Shows maximum N lines for critical/warning matches"))
        .arg(Arg::with_name("bytes").short("b").long("limit-bytes").value_name("B")
             .default_value("4096")
             .help("Truncates output in total to B bytes (suffixes accepted)"))
        .get_matches();

    let mut app = match App::try_from(&matches) {
        Ok(app) => app,
        Err(e) => {
            output(Err(e));
            println!("{}", matches.usage());
            process::exit(3);
        }
    };

    let exit = output(app.run());
    stdout().write(&app.out).ok();
    process::exit(exit)
}
