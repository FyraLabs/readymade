use serde::de::IntoDeserializer;

use crate::Value;

/// Implement `deserialize_*` methods that parse the string with [`str::parse`].
macro_rules! parse_scalars {
    ($($method:ident => $visit:ident),* $(,)?) => {$(
        fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: serde::de::Visitor<'de>,
        {
            visitor.$visit(self.0.parse().map_err(|e| {
                serde::de::Error::custom(format!("failed to parse {:?}: {e}", self.0))
            })?)
        }
    )*};
}

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

    parse_scalars! {
        deserialize_i8 => visit_i8,
        deserialize_i16 => visit_i16,
        deserialize_i32 => visit_i32,
        deserialize_i64 => visit_i64,
        deserialize_u8 => visit_u8,
        deserialize_u16 => visit_u16,
        deserialize_u32 => visit_u32,
        deserialize_u64 => visit_u64,
        deserialize_f32 => visit_f32,
        deserialize_f64 => visit_f64,
        deserialize_char => visit_char,
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_bool(parse_bool(self.0).ok_or_else(|| {
            serde::de::Error::custom(format!("failed to parse bool from {:?}", self.0))
        })?)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_str(self.0)
    }

    serde::forward_to_deserialize_any! {
        string bytes byte_buf unit unit_struct newtype_struct tuple
        tuple_struct struct ignored_any map
    }
}

/// Parse a boolean the same way systemd's `parse_boolean()` does.
#[must_use]
pub fn parse_bool(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "1" | "yes" | "y" | "true" | "t" | "on" => Some(true),
        "0" | "no" | "n" | "false" | "f" | "off" => Some(false),
        _ => None,
    }
}

/// Forward `deserialize_*` methods expecting a single value to [`OptVecDeserializer::last`].
macro_rules! forward_to_last {
    ($($method:ident)*) => {$(
        fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: serde::de::Visitor<'de>,
        {
            self.last()?.$method(visitor)
        }
    )*};
}

pub struct OptVecDeserializer<'a>(pub &'a [String]);

impl<'de> serde::de::Deserializer<'de> for OptVecDeserializer<'_> {
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

        struct SeqDeserializer<'a>(std::slice::Iter<'a, String>);

        impl<'de> SeqAccess<'de> for SeqDeserializer<'_> {
            type Error = serde::de::value::Error;

            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
            where
                T: serde::de::DeserializeSeed<'de>,
            {
                self.0.next().map_or_else(
                    || Ok(None),
                    |val| seed.deserialize(OptStringDeserializer(val)).map(Some),
                )
            }
        }

        visitor.visit_seq(SeqDeserializer(self.0.iter()))
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
        self.last()?.deserialize_enum(name, variants, visitor)
    }

    forward_to_last! {
        deserialize_bool deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64
        deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64 deserialize_f32
        deserialize_f64 deserialize_char deserialize_str deserialize_string
        deserialize_identifier
    }

    serde::forward_to_deserialize_any! {
        bytes byte_buf unit unit_struct newtype_struct tuple
        tuple_struct struct ignored_any map
    }
}

impl OptVecDeserializer<'_> {
    /// When a single value is expected but the key was repeated, the last assignment wins, like
    /// it does for most systemd settings.
    fn last(&self) -> Result<OptStringDeserializer<'_>, serde::de::value::Error> {
        self.0
            .last()
            .map(|s| OptStringDeserializer(s))
            .ok_or_else(|| serde::de::Error::custom("empty sequence"))
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
    pub map: &'a std::collections::HashMap<String, Value>,
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
        // the section exists, otherwise its key would not have been visited at all
        visitor.visit_some(self)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        struct KeysMap<'a> {
            iter: std::collections::hash_map::Iter<'a, String, Value>,
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
                        Value::Array(arr) => seed.deserialize(OptVecDeserializer(arr)),
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
