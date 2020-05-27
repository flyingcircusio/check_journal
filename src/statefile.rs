use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// This is what actually gets written to disk
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct State {
    cursor: String,
}

/// Wrapper for handling load/update
#[derive(Debug, Default)]
pub struct Statefile {
    filename: PathBuf,
    state: State,
}

impl Statefile {
    pub fn load<P: AsRef<Path>>(filename: P) -> Result<Self> {
        let filename = filename.as_ref().to_owned();
        let s: Result<State> = File::open(&filename)
            .context("open file")
            .and_then(|f| serde_yaml::from_reader(f).context("parse YAML"));
        if let Ok(state) = s {
            return Ok(Self { filename, state });
        }
        // Ignore error and try to touch the file
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filename)
            .context("Cannot create state file")?;
        Ok(Self {
            filename,
            state: State::default(),
        })
    }

    pub fn update_cursor(&mut self, new_cursor: &str) -> Result<()> {
        self.state.cursor = new_cursor.to_owned();
        serde_yaml::to_writer(
            File::create(&self.filename).context("Failed to open state file for writing")?,
            &self.state,
        )?;
        Ok(())
    }

    pub fn get_cursor(&self) -> &str {
        &self.state.cursor
    }
}
