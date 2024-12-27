//! Systemd INI-esque configuration file parser, incomplete.
//!

use serde::{
    de::{self, IntoDeserializer},
    forward_to_deserialize_any,
};
use std::collections::BTreeMap;

// we can probably have duplicate keys with different values
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Value {
    String(String),
    Array(Vec<String>),
}

impl Value {
    pub fn as_str(&self) -> &str {
        match self {
            Self::String(s) => s,
            Self::Array(arr) => arr.first().unwrap(),
        }
    }

    pub fn as_array(&self) -> Vec<String> {
        match self {
            Self::String(s) => vec![s.clone()],
            Self::Array(arr) => arr.clone(),
        }
    }

    /// Append a value, converting the current value to an array if necessary.
    pub fn append(&mut self, value: String) {
        match self {
            Self::String(old_value) => {
                *self = Self::Array(vec![old_value.clone(), value]);
            }
            Self::Array(arr) => arr.push(value),
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct SystemdIni {
    pub sections: BTreeMap<String, BTreeMap<String, Value>>,
}

impl SystemdIni {
    pub const fn new() -> Self {
        Self {
            sections: BTreeMap::new(),
        }
    }

    pub fn parse(&mut self, data: &str) {
        let mut current_section = String::new();
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                current_section.clear();
                if let Some(section) = line.get(1..line.len().saturating_sub(1)) {
                    section.clone_into(&mut current_section);
                }
                self.sections.entry(current_section.clone()).or_default();
            } else {
                let mut parts = line.splitn(2, '=');
                let key = parts.next().unwrap().trim().to_owned();
                let value = parts.next().unwrap().trim().to_owned();
                let section = self.sections.get_mut(&current_section).unwrap();
                section
                    .entry(key)
                    .and_modify(|e| e.append(value.clone()))
                    .or_insert_with(|| Value::String(value));
            }
        }
    }
}

// todo: Probably implement serde::de::Visitor for this or something
impl std::str::FromStr for SystemdIni {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut ini = Self::new();
        ini.parse(s);
        Ok(ini)
    }
}

impl std::fmt::Debug for SystemdIni {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (section, keys) in &self.sections {
            writeln!(f, "[{section}]")?;
            for (key, value) in keys {
                match value {
                    Value::String(s) => writeln!(f, "{key}={s}")?,
                    Value::Array(arr) => write_array(f, key, arr)?,
                }
            }
        }
        Ok(())
    }
}

fn write_array(f: &mut std::fmt::Formatter<'_>, key: &str, arr: &[String]) -> std::fmt::Result {
    for s in arr {
        writeln!(f, "{key}={s}")?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_section() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section]\nkey=value");
        assert_eq!(ini.sections["Section"]["key"].as_str(), "value");
    }

    #[test]
    fn test_parse_multiple_sections() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section1]\nkey1=value1\n[Section2]\nkey2=value2");
        assert_eq!(ini.sections["Section1"]["key1"].as_str(), "value1");
        assert_eq!(ini.sections["Section2"]["key2"].as_str(), "value2");
    }

    #[test]
    fn test_parse_multiple_keys_in_section() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section]\nkey1=value1\nkey2=value2");
        assert_eq!(ini.sections["Section"]["key1"].as_str(), "value1");
        assert_eq!(ini.sections["Section"]["key2"].as_str(), "value2");
    }

    #[test]
    fn test_parse_duplicate_keys() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section]\nkey=value1\nkey=value2");
        assert_eq!(
            ini.sections["Section"]["key"].as_array(),
            vec!["value1", "value2"]
        );
    }

    #[test]
    fn test_parse_empty_lines() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section]\n\nkey=value\n\n");
        assert_eq!(ini.sections["Section"]["key"].as_str(), "value");
    }

    #[test]
    fn test_parse_whitespace_lines() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section]\n  \nkey = value\n  ");
        assert_eq!(ini.sections["Section"]["key"].as_str(), "value");
    }

    #[test]
    fn test_serde() {
        let ini = "[Section1]
key1=value1
key2=value2
[Section2]
key3=value3
key3=value4
";
        let parsed: SystemdIni = ini.parse().unwrap();

        println!("{parsed:#?}");
        assert_eq!(parsed.sections["Section1"]["key1"].as_str(), "value1");
        assert_eq!(parsed.sections["Section1"]["key2"].as_str(), "value2");
        assert_eq!(
            parsed.sections["Section2"]["key3"].as_array(),
            &["value3", "value4"]
        );
    }
}
