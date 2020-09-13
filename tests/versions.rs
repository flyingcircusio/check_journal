//! Verify version numbers used in various parts of the project are the same.

use version_sync::assert_contains_regex;

#[test]
fn default_nix() {
    assert_contains_regex!("default.nix", r#"version = "{version}";"#);
}

#[test]
fn snapcraft() {
    assert_contains_regex!("snap/snapcraft.yaml", r#"version: '{version}'"#);
}
