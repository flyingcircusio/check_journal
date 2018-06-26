#[macro_use]
extern crate clap;
#[macro_use]
extern crate derive_error_chain;
extern crate error_chain;
extern crate nix;
extern crate regex;
extern crate reqwest;
extern crate subprocess;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_yaml;
#[macro_use]
extern crate lazy_static;
extern crate lazycell;

mod check;
mod rules;
#[cfg(test)]
mod tests;
mod timeout;

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

    #[error_chain(foreign)]
    Regex(regex::Error),

    Msg(String),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_, _| "critical check result""#)]
    #[error_chain(
        display = r#"|crit, warn|
                  write!(f, "{} critical, {} warning line(s) found", crit, warn)"#
    )]
    Critical(usize, usize),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "warning check result""#)]
    #[error_chain(display = r#"|warn| write!(f, "{} warning line(s) found", warn)"#)]
    Warning(usize),

    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "unexpected journalctl exit""#)]
    #[error_chain(display = r#"|s| write!(f, "journalctl failed with exit code {:?}", s)"#)]
    Journal(ExitStatus),
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
            Arg::from_usage("-j, --journalctl <PATH> 'Executable to call'")
                .default_value("journalctl"),
        )
        .arg(
            Arg::from_usage("-t, --timeout <T> 'Aborts check execution after T seconds'")
                .default_value("60"),
        )
        .arg(
            Arg::from_usage(
                "-s, --span <TIMESPEC> 'Reads journal entries from the last TIMESPEC (time \
                 suffixes accepted)'",
            ).default_value("601s"),
        )
        .arg(
            Arg::from_usage("-l, --lines <N> 'Shows maximum N lines for critical/warning matches'")
                .default_value("25")
                .alias("limit"),
        )
        .arg(
            Arg::from_usage("-b, --bytes <B> 'Truncates output to B bytes total'")
                .default_value("8192"),
        )
        .arg(Arg::from_usage("-v, --verbose '(ignored)'").hidden(true))
        .arg(Arg::from_usage(
            "<RULES_YAML> 'match patterns (file name or URL)'",
        ))
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
