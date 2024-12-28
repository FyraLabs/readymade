//! Systemd INI-esque configuration file parser, incomplete.
//!

use serde::de::IntoDeserializer;
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
#[derive(Clone, PartialEq, Eq, Default)]
pub struct SystemdIni {
    pub sections: BTreeMap<String, BTreeMap<String, Value>>,
}

impl SystemdIni {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(&mut self, data: &str) {
        let mut current_section = String::new();
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
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
            iter: std::collections::btree_map::Iter<
                'a,
                String,
                std::collections::BTreeMap<String, Value>,
            >,
            current: Option<(&'a String, &'a std::collections::BTreeMap<String, Value>)>,
        }

        impl<'de, 'a> serde::de::MapAccess<'de> for SectionsMap<'a> {
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
                    seed.deserialize(SectionDeserializer { map: v })
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

struct SectionDeserializer<'a> {
    map: &'a std::collections::BTreeMap<String, Value>,
}

impl<'de, 'a> serde::de::Deserializer<'de> for SectionDeserializer<'a> {
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
        if self.map.is_empty() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        struct KeysMap<'a> {
            iter: std::collections::btree_map::Iter<'a, String, Value>,
            current: Option<(&'a String, &'a Value)>,
        }

        impl<'de, 'a> serde::de::MapAccess<'de> for KeysMap<'a> {
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
                if let Some((_, val)) = self.current.take() {
                    match val {
                        Value::String(s) => seed.deserialize(s.as_str().into_deserializer()),
                        Value::Array(arr) => seed.deserialize(arr.clone().into_deserializer()),
                    }
                } else {
                    Err(serde::de::Error::custom("Value missing"))
                }
            }
        }

        visitor.visit_map(KeysMap {
            iter: self.map.iter(),
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
        println!("{:#?}", test);
        assert!(test.partition.copy_blocks.is_some());
    }
}
