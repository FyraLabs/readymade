use serde::de::IntoDeserializer;

use crate::Value;

pub struct OptStringDeserializer<'a>(pub &'a str);

impl<'de> serde::de::Deserializer<'de> for OptStringDeserializer<'_> {
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

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        EnumDeserializer(self.0).deserialize_enum(name, variants, visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        // Create a single-element sequence from the string
        struct SingleValueSeq<'a>(&'a str);

        impl<'de> serde::de::SeqAccess<'de> for SingleValueSeq<'_> {
            type Error = serde::de::value::Error;

            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
            where
                T: serde::de::DeserializeSeed<'de>,
            {
                if self.0.is_empty() {
                    Ok(None)
                } else {
                    let value = self.0;
                    self.0 = "";
                    seed.deserialize(OptStringDeserializer(value)).map(Some)
                }
            }
        }

        visitor.visit_seq(SingleValueSeq(self.0))
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u32(
            self.0
                .parse()
                .map_err(|_| serde::de::Error::custom("failed to parse u32"))?,
        )
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u64(
            self.0
                .parse()
                .map_err(|_| serde::de::Error::custom("failed to parse u64"))?,
        )
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i32(
            self.0
                .parse()
                .map_err(|_| serde::de::Error::custom("failed to parse i32"))?,
        )
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i64(
            self.0
                .parse()
                .map_err(|_| serde::de::Error::custom("failed to parse i64"))?,
        )
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 f32 f64 char string
        bytes byte_buf unit unit_struct newtype_struct tuple
        tuple_struct struct ignored_any map
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_str(self.0)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u8(
            self.0
                .parse()
                .map_err(|_| serde::de::Error::custom("failed to parse u8"))?,
        )
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u16(
            self.0
                .parse()
                .map_err(|_| serde::de::Error::custom("failed to parse u16"))?,
        )
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
                self.0.next().map_or_else(
                    || Ok(None),
                    |val| seed.deserialize(OptStringDeserializer(&val)).map(Some),
                )
            }
        }

        visitor.visit_seq(SeqDeserializer(self.0.into_iter()))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        // When a string is expected but we have a sequence, use the first element
        self.0.first().map_or_else(
            || Err(serde::de::Error::custom("empty sequence")),
            |first| visitor.visit_str(first),
        )
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char
        bytes byte_buf unit unit_struct newtype_struct tuple
        tuple_struct struct enum identifier ignored_any map
    }
}

/// Deserializer for converting strings to enum variants
pub struct EnumDeserializer<'a>(&'a str);

impl<'de> serde::de::Deserializer<'de> for EnumDeserializer<'_> {
    type Error = serde::de::value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_enum("", &[], visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_enum(self)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct struct map option identifier ignored_any
    }
}

impl<'de> serde::de::EnumAccess<'de> for EnumDeserializer<'_> {
    type Error = serde::de::value::Error;
    type Variant = UnitOnlyVariantAccess;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(self.0.into_deserializer())?;
        Ok((variant, UnitOnlyVariantAccess))
    }
}

/// Helper struct for unit-only enum variants
pub struct UnitOnlyVariantAccess;

impl<'de> serde::de::VariantAccess<'de> for UnitOnlyVariantAccess {
    type Error = serde::de::value::Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        Err(serde::de::Error::custom(
            "newtype variants are not supported",
        ))
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(serde::de::Error::custom("tuple variants are not supported"))
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(serde::de::Error::custom(
            "struct variants are not supported",
        ))
    }
}

pub struct SectionDeserializer<'a> {
    pub map: &'a std::collections::BTreeMap<String, Value>,
}

impl<'de> serde::de::Deserializer<'de> for SectionDeserializer<'_> {
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

        impl<'de> serde::de::MapAccess<'de> for KeysMap<'_> {
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
