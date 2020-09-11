use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process;
use structopt::clap::crate_name;
use structopt::StructOpt;

mod check;
mod rules;
use check::{Check, Status};

/// Nagios/Icinga compatible plugin to search `journalctl` output for matching lines
#[derive(Debug, Default, StructOpt)]
pub struct Opt {
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
    /// "journalctl" executable to call
    #[structopt(
        short,
        long,
        value_name = "PATH",
        default_value = option_env!("JOURNALCTL").unwrap_or("journalctl")
    )]
    journalctl: String,
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

fn run() -> Result<i32, anyhow::Error> {
    let mut check = Check::new(Opt::from_args())?;
    let out = check.run();
    let exitcode = match out.status? {
        Status::Ok(summary) => {
            println!("{} OK - {}", crate_name!(), summary);
            0
        }
        Status::Warning(n) => {
            println!("{} WARNING - {} warning line(s) found", crate_name!(), n);
            1
        }
        Status::Critical(c, w) => {
            println!(
                "{} CRITICAL - {} critical, {} warning line(s) found",
                crate_name!(),
                c,
                w
            );
            2
        }
    };
    stdout().write(&out.message).ok();
    Ok(exitcode)
}

fn main() {
    match run() {
        Ok(exitcode) => process::exit(exitcode),
        Err(err) => {
            println!("{} UNKNOWN - {:?}", crate_name!(), err);
            process::exit(3);
        }
    }
}
