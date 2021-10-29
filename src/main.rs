use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process;
use structopt::clap::crate_name;
use structopt::StructOpt;

mod check;
mod rules;
#[cfg(test)]
mod tests;
mod utils;

use check::{Check, Status};
use utils::Chrono;

/// Nagios/Icinga compatible plugin to search `journalctl` output for matching lines
#[derive(Debug, Default, StructOpt)]
pub struct Opt {
    /// Reads journal entries from the last TIMESPEC (time suffixes accepted)
    ///
    /// This option applies only if no previous position could be read from the state file
    #[structopt(short, long, default_value = "600s", value_name = "TIMESPEC")]
    span: String,
    /// Truncates report of critical/warning matches to N lines each
    #[structopt(short, long, alias = "lines", default_value = "50", value_name = "N")]
    limit: usize,
    /// Does not truncate output (opposite of --limit)
    #[structopt(short = "L", long, conflicts_with = "limit")]
    no_limit: bool,
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
    journalctl: PathBuf,
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
    let out = check.evaluate(check.exec_journalctl()?)?;
    let timestamp: String = format!(
        "Start ==> Timestamp: {}\n",
        Chrono::get_utc_timestamp_as_rfc2822()
    );

    let exitcode = match out.status {
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
    println!("{}", timestamp);

    write!(stdout(), "{}", &out.message).ok();

    let timestamp_end: String = format!(
        "\nFinished ==> Timestamp: {}",
        Chrono::get_utc_timestamp_as_rfc2822()
    );
    println!("{}", timestamp_end);
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
