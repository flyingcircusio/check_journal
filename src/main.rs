mod check;
mod rules;

use crate::check::Check;

use anyhow::Result;
use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process;
use structopt::clap::crate_name;
use structopt::StructOpt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ok(String),
    Critical(usize, usize),
    Warning(usize),
}

fn output(res: Result<Status>) -> i32 {
    let (keyword, summary, exitcode) = match res {
        Ok(Status::Ok(summary)) => ("OK", summary, 0),
        Ok(Status::Warning(n)) => ("WARNING", format!("{} warning line(s) found", n), 1),
        Ok(Status::Critical(n, m)) => (
            "CRITICAL",
            format!("{} critical, {} warning line(s) found", n, m),
            2,
        ),
        Err(e) => ("UNKNOWN", format!("{:?}", e), 3),
    };
    println!("{} {} - {}", crate_name!(), keyword, summary);
    exitcode
}

#[derive(Debug, Default, StructOpt)]
pub struct Opt {
    /// Executable to call
    #[structopt(short, long, default_value = "journalctl")]
    journalctl: String,
    /// Reads journal entries from the last TIMESPEC (time suffixes accepted)
    #[structopt(short, long, default_value = "601s", value_name = "TIMESPEC")]
    span: String,
    /// Shows maximum N lines for critical/warning matches
    #[structopt(short, long, alias = "limit", default_value = "25", value_name = "N")]
    lines: usize,
    /// Truncates output to B bytes total
    #[structopt(short, long, default_value = "8192", value_name = "B")]
    bytes: usize,
    #[structopt(short, long, hidden = true)]
    verbose: bool,
    /// match patterns from file or URL
    #[structopt(parse(from_os_str), value_name = "RULES_YAML")]
    rules_yaml: PathBuf,
}

fn main() {
    let mut app = match Check::new(Opt::from_args()) {
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
