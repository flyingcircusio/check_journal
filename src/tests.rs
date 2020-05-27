//! Generic test setup

use lazy_static::lazy_static;
use std::path::{Path, PathBuf};

lazy_static! {
    pub static ref FIXTURES: PathBuf = Path::new(file!()).parent().unwrap().join("../fixtures");
}
