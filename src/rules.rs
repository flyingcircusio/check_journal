use super::{Result, ResultExt};
use regex::bytes::Regex;
use reqwest;
use serde_yaml;
use std::fs::File;
use std::io::{BufReader, Read};

#[derive(Debug, Default, Deserialize)]
pub struct RulesFile {
    criticalpatterns: Vec<String>,
    criticalexceptions: Vec<String>,
    warningpatterns: Vec<String>,
    warningexceptions: Vec<String>,
}

#[derive(Debug, Default)]
pub struct RegexSet {
    matches: Vec<Regex>,
    except: Vec<Regex>,
}

impl RegexSet {
    pub fn is_match(&self, line: &[u8]) -> bool {
        self.matches.iter().any(|r| r.is_match(line))
            && !self.except.iter().any(|r| r.is_match(line))
    }
}

fn compile(rules: &[String], which: &str) -> Result<Vec<Regex>> {
    rules
        .iter()
        .enumerate()
        .map(|(i, r)| Regex::new(r).chain_err(|| format!("while loading {} rule {}", which, i + 1)))
        .collect()
}

#[derive(Debug, Default)]
pub struct Rules {
    crit: RegexSet,
    warn: RegexSet,
}

impl Rules {
    pub fn match_push<'a>(
        &self,
        line: &'a [u8],
        crit: &mut Vec<&'a [u8]>,
        warn: &mut Vec<&'a [u8]>,
    ) {
        if self.crit.is_match(&line) {
            crit.push(line);
        } else if self.warn.is_match(&line) {
            warn.push(line);
        }
    }

    pub fn try_from<R: Read>(rdr: R) -> Result<Self> {
        let r: RulesFile =
            serde_yaml::from_reader(BufReader::new(rdr)).chain_err(|| "YAML parse error")?;
        Ok(Self {
            crit: RegexSet {
                matches: compile(&r.criticalpatterns, "critical patterns")?,
                except: compile(&r.criticalexceptions, "critical exceptions")?,
            },
            warn: RegexSet {
                matches: compile(&r.warningpatterns, "warning patterns")?,
                except: compile(&r.warningexceptions, "warning exceptions")?,
            },
        })
    }

    pub fn load(source: &str) -> Result<Self> {
        if source.contains("://") {
            Self::try_from(reqwest::get(source)
                .chain_err(|| "download error")?
                .error_for_status()
                .chain_err(|| "HTTP error")?)
        } else {
            Self::try_from(File::open(source).chain_err(|| format!("cannot open rules file '{}'", source))?)
        }
    }
}
