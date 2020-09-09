mod check;
mod rules;

use crate::check::{Check, Status};

use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process;
use structopt::clap::crate_name;
use structopt::StructOpt;

#[derive(Debug, Default, StructOpt)]
pub struct Opt {
    /// Executable to call
    #[structopt(short, long, default_value = "journalctl")]
    journalctl: String,
    /// Reads journal entries from the last TIMESPEC (time suffixes accepted)
    ///
    /// This option applies only if no previous position could be read from the state file
    #[structopt(
        short,
        long,
        alias = "since",
        default_value = "600s",
        value_name = "TIMESPEC"
    )]
    span: String,
    /// Shows maximum N lines for critical/warning matches
    #[structopt(short, long, alias = "limit", default_value = "25", value_name = "N")]
    lines: usize,
    /// Saves last log position for exact resume
    #[structopt(short = "f", long, value_name = "PATH")]
    statefile: Option<PathBuf>,
    // ignored, retained for compatibility
    #[structopt(short, long, hidden = true)]
    verbose: bool,
    /// Match patterns from file or URL
    ///
    /// In case of an URL, it will be downloaded automatically on each run. On download errors,
    /// this plugin will exit with an UNKNOWN state.
    #[structopt(parse(from_os_str), value_name = "RULES_YAML")]
    rules_yaml: PathBuf,
}

fn main() {
    let mut check = match Check::new(Opt::from_args()) {
        Ok(app) => app,
        Err(e) => {
            println!("{} UNKNOWN - {:?}", crate_name!(), e);
            process::exit(3);
        }
    };
    let out = check.run();
    let exitcode = match out.status {
        Ok(Status::Ok(summary)) => {
            println!("{} OK - {}", crate_name!(), summary);
            0
        }
        Ok(Status::Warning(n)) => {
            println!("{} WARNING - {} warning line(s) found", crate_name!(), n);
            1
        }
        Ok(Status::Critical(c, w)) => {
            println!(
                "{} CRITICAL - {} critical, {} warning line(s) found",
                crate_name!(),
                c,
                w
            );
            2
        }
        Err(e) => {
            println!("{} UNKNOWN - {}", crate_name!(), e);
            3
        }
    };
    stdout().write(&out.message).ok();
    process::exit(exitcode);
}
