use nix::libc;
use std::sync::{Arc, Mutex};
use nix::sys::signal::*;
use nix::unistd::alarm;
use std::process;
use std::ops::{Deref, DerefMut};

lazy_static!{
    static ref T: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
}

extern "C" fn hdl(_: libc::c_int) {
    println!("{} UNKNOWN - timed out after {}s", crate_name!(), T.lock().unwrap().deref());
    process::exit(3);
}

pub fn install(timeout: u32) {
    *T.lock().unwrap().deref_mut() = timeout;
    unsafe {
        sigaction(
            Signal::SIGALRM,
            &SigAction::new(SigHandler::Handler(hdl), SaFlags::empty(), SigSet::empty()),
        )
    }.expect("failed to set signal handler");
    alarm::set(timeout as libc::c_uint);
}
