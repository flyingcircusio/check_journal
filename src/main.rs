#[macro_use]
extern crate clap;
#[macro_use]
extern crate derive_error_chain;
extern crate error_chain;
extern crate regex;
extern crate subprocess;
#[macro_use]
extern crate serde_derive;
extern crate reqwest;
extern crate serde;
extern crate serde_yaml;

mod check;
mod rules;

use clap::Arg;
use std::error::Error as ErrorTrait;
use std::io::{stdout, Write};
use std::process;
use subprocess::ExitStatus;

#[derive(Debug, ErrorChain)]
pub enum ErrorKind {
    #[error_chain(foreign)]
    Fmt(std::fmt::Error),

    #[error_chain(foreign)]
    Io(std::io::Error),

    #[error_chain(foreign)]
    Clap(clap::Error),

    #[error_chain(foreign)]
    Popen(subprocess::PopenError),

    #[error_chain(foreign)]
    YAML(serde_yaml::Error),

    #[error_chain(foreign)]
    Reqwest(reqwest::Error),

    Msg(String),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_, _| "critical check result""#)]
    #[error_chain(display = r#"|crit, warn|
                  write!(f, "{} critical, {} warning line(s) found", crit, warn)"#)]
    Critical(usize, usize),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "warning check result""#)]
    #[error_chain(display = r#"|warn| write!(f, "{} warning line(s) found", warn)"#)]
    Warning(usize),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "unexpected journalctl exit""#)]
    #[error_chain(display = r#"|s| write!(f, "journalctl failed with exit code {:?}", s)"#)]
    Journal(ExitStatus),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "regex error""#)]
    #[error_chain(display = r#"|re| write!(f, "regex parse failure: {}", re)"#)]
    Regex(i32),
}

fn output(res: Result<String>) -> i32 {
    let (keyword, summary, exitcode) = match res {
        Ok(summary) => ("OK", summary, 0),
        Err(e) => {
            let summary = if let Some(cause) = e.cause() {
                format!("{}: {}", e, cause)
            } else {
                format!("{}", e)
            };
            match e.kind() {
                ErrorKind::Warning(_) => ("WARNING", summary, 1),
                ErrorKind::Critical(_, _) => ("CRITICAL", summary, 2),
                _ => ("UNKNOWN", summary, 3),
            }
        }
    };
    println!("{} {} - {}", crate_name!(), keyword, summary);
    exitcode
}

fn main() {
    let matches = app_from_crate!()
        .arg(
            Arg::with_name("rules")
                .value_name("RULES")
                .required(true)
                .help("YAML logcheck rules, local file name or URL"),
        )
        .arg(
            Arg::with_name("journalctl")
                .long("journalctl")
                .default_value("journalctl")
                .help("Path to journalctl executable"),
        )
        .arg(
            Arg::with_name("timeout")
                .short("t")
                .long("timeout")
                .value_name("T")
                .default_value("60")
                .help("Aborts check execution after T seconds"),
        )
        .arg(
            Arg::with_name("span")
                .short("s")
                .long("span")
                .value_name("EXPR")
                .default_value("600")
                .help("Journal search time span (seconds; time suffixes accepted)"),
        )
        .arg(
            Arg::with_name("lines")
                .short("l")
                .long("limit-lines")
                .value_name("N")
                .default_value("100")
                .help("Shows maximum N lines for critical/warning matches"),
        )
        .arg(
            Arg::with_name("bytes")
                .short("b")
                .long("limit-bytes")
                .value_name("B")
                .default_value("4096")
                .help("Truncates output in total to B bytes"),
        )
        .get_matches();

    let mut app = match check::Check::try_from(&matches) {
        Ok(app) => app,
        Err(e) => {
            output(Err(e));
            process::exit(3);
        }
    };

    let mut out = Vec::with_capacity(8192);
    let exit = output(app.run(&mut out));
    stdout().write(&out).ok();
    process::exit(exit)
}
