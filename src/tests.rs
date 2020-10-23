//! Generic test setup

use crate::rules::Rules;

use lazy_static::lazy_static;
use std::path::{Path, PathBuf};

pub fn fixture(item: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(item)
}

lazy_static! {
    pub static ref RULES: Rules =
        Rules::load(fixture("rules.yaml")).expect("failed to load test rules");
}
