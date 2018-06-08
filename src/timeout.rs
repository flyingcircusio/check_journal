use lazycell::AtomicLazyCell;
use nix::libc;
use nix::sys::signal::*;
use nix::unistd::alarm;
use std::process;
use {Result, ResultExt};

lazy_static! {
    static ref T: AtomicLazyCell<u32> = AtomicLazyCell::new();
}

extern "C" fn hdl(_: libc::c_int) {
    println!(
        "{} UNKNOWN - timed out after {}s",
        crate_name!(),
        T.get().unwrap_or_default()
    );
    process::exit(3);
}

pub fn install(timeout: u32) -> Result<()> {
    T.fill(timeout).expect("BUG: trying to set up alarm twice");
    unsafe {
        sigaction(
            Signal::SIGALRM,
            &SigAction::new(SigHandler::Handler(hdl), SaFlags::empty(), SigSet::empty()),
        )
    }.chain_err(|| "failed to set signal handler")?;
    alarm::set(timeout as libc::c_uint);
    Ok(())
}
