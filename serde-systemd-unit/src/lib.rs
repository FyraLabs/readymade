//! Systemd INI-esque configuration file parser, incomplete.
//!

use itertools::Itertools;
mod de;
pub mod parser;
mod se;
use de::SectionDeserializer;
use parser::Err;
pub use se::to_string;
use serde::de::IntoDeserializer;
use std::collections::HashMap;
/// Parse a systemd unit file from string, returning a `SystemdIni` struct.
///
/// # Errors
///
/// Returns an error if the string cannot be parsed as a valid systemd unit file.
///
pub fn parse(s: &str) -> Result<SystemdIni, Err> {
    Ok(SystemdIni {
        sections: parser::parse_str(s).map(|h| {
            h.into_iter()
                .map(|(section, entries)| {
                    (section, entries.into_iter().into_grouping_map().collect())
                })
                .map(|(section, entries)| {
                    (
                        section,
                        entries
                            .into_iter()
                            .map(|(k, v): (_, Vec<_>)| (k, Value::from_vec(&v)))
                            .collect(),
                    )
                })
                .collect()
        })?,
    })
}

// we can probably have duplicate keys with different values
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Value {
    String(String),
    Array(Vec<String>),
}

impl Value {
    #[must_use]
    pub fn from_vec(v: &[String]) -> Self {
        if let [one] = v {
            Self::String(one.clone())
        } else {
            Self::Array(v.to_owned())
        }
    }
    /// Returns the string value or the first element of the array.
    ///
    /// # Panics
    /// Panics if the value is an empty array.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::String(s) => s,
            Self::Array(arr) => arr.first().unwrap(),
        }
    }

    #[must_use]
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

#[derive(Clone, PartialEq, Eq, Default)]
pub struct SystemdIni {
    pub sections: HashMap<String, HashMap<String, Value>>,
}

impl std::fmt::Display for SystemdIni {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (k, v) in &self.sections {
            f.write_fmt(format_args!("[{k}]\n"))?;
            for (k, v) in v {
                for s in (match v {
                    Value::String(s) => {
                        Box::new(std::iter::once(&**s)) as Box<dyn Iterator<Item = &str>>
                    }
                    Value::Array(v) => Box::new(v.iter().map(String::as_str)),
                } as Box<dyn Iterator<Item = &str>>)
                {
                    f.write_fmt(format_args!("{k}={s}\n"))?;
                }
            }
        }
        Ok(())
    }
}

impl SystemdIni {
    /// Parse a systemd unit file from a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string cannot be parsed as a valid systemd unit file.
    pub fn parse(&mut self, s: &str) -> Result<(), Err> {
        *self = parse(s)?;
        Ok(())
    }
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::str::FromStr for SystemdIni {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse(s).map_err(|e| format!("{e:?}"))
    }
}

impl<'de> serde::de::Deserializer<'de> for &'de SystemdIni {
    type Error = serde::de::value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if self.sections.is_empty() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        struct SectionsMap<'a> {
            iter: std::collections::hash_map::Iter<'a, String, HashMap<String, Value>>,
            current: Option<(&'a String, &'a HashMap<String, Value>)>,
        }

        impl<'de> serde::de::MapAccess<'de> for SectionsMap<'_> {
            type Error = serde::de::value::Error;

            fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
            where
                K: serde::de::DeserializeSeed<'de>,
            {
                if let Some((k, _)) = self.current {
                    seed.deserialize(k.as_str().into_deserializer()).map(Some)
                } else if let Some((k, v)) = self.iter.next() {
                    self.current = Some((k, v));
                    seed.deserialize(k.as_str().into_deserializer()).map(Some)
                } else {
                    Ok(None)
                }
            }

            fn next_value_seed<Vv>(&mut self, seed: Vv) -> Result<Vv::Value, Self::Error>
            where
                Vv: serde::de::DeserializeSeed<'de>,
            {
                if let Some((_, v)) = self.current.take() {
                    seed.deserialize(SectionDeserializer {
                        map: &v.clone().into_iter().collect(),
                    })
                } else {
                    Err(serde::de::Error::custom("Value missing"))
                }
            }
        }

        visitor.visit_map(SectionsMap {
            iter: self.sections.iter(),
            current: None,
        })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct struct enum identifier ignored_any
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

/// Deserialize a type from a string containing systemd unit file format.
///
/// # Errors
/// Returns an error if the string cannot be parsed as a valid systemd unit file,
/// or if the deserialization fails due to missing or incorrectly typed values.
pub fn from_str<T>(s: &str) -> Result<T, serde::de::value::Error>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let ini = s.parse::<SystemdIni>().map_err(serde::de::Error::custom)?;
    serde::Deserialize::deserialize(&ini)
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
        ini.parse("[Section]\nkey=value").unwrap();
        assert_eq!(ini.sections["Section"]["key"].as_str(), "value");
    }

    #[test]
    fn test_parse_multiple_sections() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section1]\nkey1=value1\n[Section2]\nkey2=value2")
            .unwrap();
        assert_eq!(ini.sections["Section1"]["key1"].as_str(), "value1");
        assert_eq!(ini.sections["Section2"]["key2"].as_str(), "value2");
    }

    #[test]
    fn test_parse_multiple_keys_in_section() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section]\nkey1=value1\nkey2=value2").unwrap();
        assert_eq!(ini.sections["Section"]["key1"].as_str(), "value1");
        assert_eq!(ini.sections["Section"]["key2"].as_str(), "value2");
    }

    #[test]
    fn test_parse_duplicate_keys() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section]\nkey=value1\nkey=value2").unwrap();
        assert_eq!(
            ini.sections["Section"]["key"].as_array(),
            vec!["value1", "value2"]
        );
    }

    #[test]
    fn test_parse_empty_lines() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section]\n\nkey=value\n\n").unwrap();
        assert_eq!(ini.sections["Section"]["key"].as_str(), "value");
    }

    #[test]
    fn test_parse_whitespace_lines() {
        let mut ini = SystemdIni::new();
        ini.parse("[Section]\n  \nkey = value\n  ").unwrap();
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

    #[derive(serde::Deserialize, Debug)]
    struct Test {
        section1: Section1,
    }

    #[derive(serde::Deserialize, Debug)]
    struct Section1 {
        key1: String,
        key2: String,
    }

    #[test]
    fn test_deserialize() {
        let ini = "[section1]
key1=value1
key2=value2
";
        let test: Test = from_str(ini).unwrap();
        assert_eq!(test.section1.key1, "value1");
        assert_eq!(test.section1.key2, "value2");
    }

    #[derive(serde::Deserialize, Debug)]
    struct Section2 {
        key3: Vec<String>,
    }

    #[derive(serde::Deserialize, Debug)]
    struct Test2 {
        section2: Section2,
    }

    #[test]
    fn test_deserialize_array() {
        let ini = "[section2]
key3=value1
key3=value2
";
        let test: Test2 = from_str(ini).unwrap();
        assert_eq!(test.section2.key3, vec!["value1", "value2"]);
    }
    #[test]
    fn test_deserialize_array_single() {
        let ini = "[section2]
key3=value1
";
        let test: Test2 = from_str(ini).unwrap();
        assert_eq!(test.section2.key3, vec!["value1"]);
    }

    #[test]
    fn test_deserialize_repart() {
        #[derive(serde::Deserialize, Debug)]
        #[serde(rename_all = "PascalCase")]
        struct Repart {
            partition: Partition,
        }
        #[derive(serde::Deserialize, Debug)]
        #[serde(rename_all = "PascalCase")]
        struct Partition {
            copy_blocks: Option<String>,
        }

        let file = include_str!("../test/submarine.conf");

        let test: Repart = from_str(file).unwrap();
        println!("{test:#?}");
        assert!(test.partition.copy_blocks.is_some());
    }

    #[test]
    fn test_deserialize_option() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct TestOption {
            section: SectionWithOption,
        }

        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct SectionWithOption {
            required: String,
            optional: Option<String>,
            missing: Option<String>,
        }

        let ini = "[section]\nrequired=value\noptional=optvalue\n";
        let test: TestOption = from_str(ini).unwrap();
        assert_eq!(test.section.required, "value");
        assert_eq!(test.section.optional, Some("optvalue".to_owned()));
        assert_eq!(test.section.missing, None);
    }

    #[test]
    fn test_deserialize_enum() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        enum Mode {
            Read,
            Write,
            ReadWrite,
        }

        #[derive(Debug, serde::Deserialize)]
        struct TestEnum {
            section: SectionWithEnum,
        }

        #[derive(Debug, serde::Deserialize)]
        struct SectionWithEnum {
            mode: Mode,
        }

        let ini = "[section]\nmode=ReadWrite\n";
        let test: TestEnum = from_str(ini).unwrap();
        assert_eq!(test.section.mode, Mode::ReadWrite);
    }

    #[test]
    fn test_deserialize_enum_array() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        enum Mode {
            Read,
            Write,
            ReadWrite,
        }

        #[derive(Debug, serde::Deserialize)]
        struct TestEnum {
            section: SectionWithEnum,
        }

        #[derive(Debug, serde::Deserialize)]
        struct SectionWithEnum {
            mode: Mode,
            modes: Vec<Mode>,
        }

        let ini = "[section]\nmode=ReadWrite\nmodes=Read\nmodes=Write\n";
        let test: TestEnum = from_str(ini).unwrap();
        assert_eq!(test.section.mode, Mode::ReadWrite);
        assert_eq!(test.section.modes, vec![Mode::Read, Mode::Write]);
    }

    #[test]
    fn ultimate_sanity_test() {
        let input = include_str!("../test/sanitytest.conf");
        let test: SystemdIni = input.parse().unwrap();

        println!("{test:#?}");
    }
}
