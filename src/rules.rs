use super::{Result, ResultExt};
use regex::bytes::Regex;
use reqwest;
use serde_yaml;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

fn compile(rules: &[String], which: &str) -> Result<Vec<Regex>> {
    rules
        .iter()
        .enumerate()
        .map(|(i, r)| Regex::new(r).chain_err(|| format!("while loading {} rule {}", which, i + 1)))
        .collect()
}

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

#[derive(Debug, Default)]
pub struct Rules {
    pub crit: RegexSet,
    pub warn: RegexSet,
}

impl Rules {
    fn try_from(source: &RulesFile) -> Result<Self> {
        Ok(Self {
            crit: RegexSet {
                matches: compile(&source.criticalpatterns, "critical patterns")?,
                except: compile(&source.criticalexceptions, "critical exceptions")?,
            },
            warn: RegexSet {
                matches: compile(&source.warningpatterns, "warning patterns")?,
                except: compile(&source.warningexceptions, "warning exceptions")?,
            },
        })
    }

    fn parse<R: Read>(rdr: R) -> Result<Self> {
        let r: RulesFile =
            serde_yaml::from_reader(BufReader::new(rdr)).chain_err(|| "YAML parse error")?;
        Self::try_from(&r)
    }

    pub fn load<P: AsRef<Path>>(source: P) -> Result<Self> {
        let source = source.as_ref();
        let source_str = source.to_string_lossy();
        if source_str.contains("://") {
            let s: &str = source.to_str().ok_or("URL not valid UTF-8")?;
            Self::parse(reqwest::get(s)
                .chain_err(|| "download error")?
                .error_for_status()
                .chain_err(|| "HTTP error")?)
        } else {
            Self::parse(File::open(source)
                .chain_err(|| format!("cannot open rules file '{}'", source.display()))?)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tests::FIXTURES;

    #[test]
    fn parse_failure_should_be_reported() {
        if let Err(e) = compile(
            &["foo".to_owned(), "invalid (re".to_owned(), "bar".to_owned()][..],
            "crit",
        ) {
            assert_eq!(e.description(), "while loading crit rule 2");
        } else {
            panic!("compile() did not return error");
        }
    }

    #[test]
    fn load_from_file() {
        let r = Rules::load(FIXTURES.join("rules.yaml")).expect("load from file");
        assert_eq!(r.crit.matches.len(), 2);
        assert_eq!(r.crit.except.len(), 3);
        assert_eq!(r.warn.matches.len(), 2);
        assert_eq!(r.warn.except.len(), 4);
    }

    #[test]
    fn load_from_nonexistent_url_should_fail() {
        assert!(Rules::load("http://no.such.host.example.com/rules").is_err());
    }

    #[test]
    fn matches_and_exceptions() {
        let r = Rules::load(FIXTURES.join("rules.yaml")).expect("load from file");
        assert!(r.crit.is_match(b"0 Errors"));
        assert!(!r.crit.is_match(b"0 errors"));
        assert!(r.warn.is_match(b"some WARN foo"));
        assert!(!r.warn.is_match(b"WARN: node[1234]: Exception in function"))
    }
}
