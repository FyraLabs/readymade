use serde::de::IntoDeserializer;

use crate::Value;

pub struct OptStringDeserializer<'a>(pub &'a str);

impl<'de, 'a> serde::de::Deserializer<'de> for OptStringDeserializer<'a> {
    type Error = serde::de::value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_str(self.0)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct struct enum identifier ignored_any map
    }
}

pub struct OptVecDeserializer(pub Vec<String>);

impl<'de> serde::de::Deserializer<'de> for OptVecDeserializer {
    type Error = serde::de::value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        use serde::de::SeqAccess;

        struct SeqDeserializer(std::vec::IntoIter<String>);

        impl<'de> SeqAccess<'de> for SeqDeserializer {
            type Error = serde::de::value::Error;

            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
            where
                T: serde::de::DeserializeSeed<'de>,
            {
                self.0.next().map_or_else(|| Ok(None), |val| seed.deserialize(OptStringDeserializer(&val)).map(Some))
            }
        }

        visitor.visit_seq(SeqDeserializer(self.0.into_iter()))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct tuple
        tuple_struct struct enum identifier ignored_any map
    }
}

pub struct SectionDeserializer<'a> {
    pub map: &'a std::collections::BTreeMap<String, Value>,
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
        // This function should check if we're looking for a specific key
        // and if that key exists in the map, rather than just checking if
        // the map is empty.

        // We don't have direct access to the field name here, so we'll
        // need to deserialize as a map and let the struct deserializer
        // handle the optionality of fields
        self.deserialize_map(visitor)
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
                if let Some((_key, val)) = self.current.take() {
                    match val {
                        Value::String(s) => seed.deserialize(OptStringDeserializer(s)),
                        Value::Array(arr) => seed.deserialize(OptVecDeserializer(arr.clone())),
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
