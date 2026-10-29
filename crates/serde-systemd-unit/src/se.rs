//! Serializer for the systemd unit format.
//!
//! The top-level value must be a struct or map of sections. Each section must in turn be a struct
//! or map of keys, whose values are scalars, `Option`s (skipped when `None`) or sequences of
//! scalars (written out as repeated keys).

use serde::{Serialize, ser};

/// Serialize the given data structures back into a string, in systemd format.
///
/// # Errors
///
/// This function will return an error if the data structure does not have the shape described in
/// the module docs, or a value contains a newline (which cannot be represented in this format).
pub fn to_string<T: Serialize>(value: &T) -> Result<String, Err> {
    let mut output = String::new();
    value.serialize(UnitSerializer {
        output: &mut output,
    })?;
    Ok(output)
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct Err(String);

impl ser::Error for Err {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Self(msg.to_string())
    }
}

fn unsupported<T>(what: &str, level: &str) -> Result<T, Err> {
    Result::Err(Err(format!("{what} is not supported as a {level}")))
}

/// Implement the `serialize_*` methods of [`ser::Serializer`] that always fail.
macro_rules! unsupported {
    ($level:literal: $($method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {$(
        fn $method(self, $(_: $ty),*) -> Result<$ret, Self::Error> {
            unsupported(stringify!($method), $level)
        }
    )*};
}

/// Implement every scalar `serialize_*` method of [`ser::Serializer`] as unsupported.
macro_rules! unsupported_scalars {
    ($level:literal) => {
        unsupported! { $level:
            serialize_bool(v: bool) -> Self::Ok;
            serialize_i8(v: i8) -> Self::Ok;
            serialize_i16(v: i16) -> Self::Ok;
            serialize_i32(v: i32) -> Self::Ok;
            serialize_i64(v: i64) -> Self::Ok;
            serialize_u8(v: u8) -> Self::Ok;
            serialize_u16(v: u16) -> Self::Ok;
            serialize_u32(v: u32) -> Self::Ok;
            serialize_u64(v: u64) -> Self::Ok;
            serialize_f32(v: f32) -> Self::Ok;
            serialize_f64(v: f64) -> Self::Ok;
            serialize_char(v: char) -> Self::Ok;
            serialize_str(v: &str) -> Self::Ok;
            serialize_bytes(v: &[u8]) -> Self::Ok;
            serialize_unit() -> Self::Ok;
            serialize_unit_struct(name: &'static str) -> Self::Ok;
            serialize_unit_variant(name: &'static str, idx: u32, variant: &'static str) -> Self::Ok;
            serialize_tuple(len: usize) -> Self::SerializeTuple;
            serialize_tuple_struct(name: &'static str, len: usize) -> Self::SerializeTupleStruct;
            serialize_tuple_variant(name: &'static str, idx: u32, variant: &'static str, len: usize) -> Self::SerializeTupleVariant;
            serialize_struct_variant(name: &'static str, idx: u32, variant: &'static str, len: usize) -> Self::SerializeStructVariant;
        }

        fn serialize_newtype_variant<T: ?Sized + Serialize>(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: &T,
        ) -> Result<Self::Ok, Self::Error> {
            unsupported("serialize_newtype_variant", $level)
        }
    };
}

/// Serializes the whole unit: a struct or map of sections.
struct UnitSerializer<'a> {
    output: &'a mut String,
}

impl<'a> ser::Serializer for UnitSerializer<'a> {
    type Ok = ();
    type Error = Err;
    type SerializeSeq = ser::Impossible<(), Err>;
    type SerializeTuple = ser::Impossible<(), Err>;
    type SerializeTupleStruct = ser::Impossible<(), Err>;
    type SerializeTupleVariant = ser::Impossible<(), Err>;
    type SerializeMap = SectionsSerializer<'a>;
    type SerializeStruct = SectionsSerializer<'a>;
    type SerializeStructVariant = ser::Impossible<(), Err>;

    unsupported_scalars!("unit");
    unsupported! { "unit":
        serialize_none() -> Self::Ok;
        serialize_seq(len: Option<usize>) -> Self::SerializeSeq;
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Err> {
        value.serialize(self)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Err> {
        value.serialize(self)
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Err> {
        Ok(SectionsSerializer {
            output: self.output,
            key: None,
        })
    }

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Err> {
        self.serialize_map(None)
    }
}

struct SectionsSerializer<'a> {
    output: &'a mut String,
    key: Option<String>,
}

impl SectionsSerializer<'_> {
    fn section<T: ?Sized + Serialize>(&mut self, name: &str, value: &T) -> Result<(), Err> {
        let mut entries = String::new();
        let present = value.serialize(SectionSerializer {
            output: &mut entries,
        })?;
        if present {
            if !self.output.is_empty() {
                self.output.push('\n');
            }
            self.output.push('[');
            self.output.push_str(name);
            self.output.push_str("]\n");
            self.output.push_str(&entries);
        }
        Ok(())
    }
}

impl ser::SerializeStruct for SectionsSerializer<'_> {
    type Ok = ();
    type Error = Err;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Err> {
        self.section(key, value)
    }

    fn end(self) -> Result<(), Err> {
        Ok(())
    }
}

impl ser::SerializeMap for SectionsSerializer<'_> {
    type Ok = ();
    type Error = Err;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Err> {
        self.key = Some(scalar(key)?);
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Err> {
        let key = self
            .key
            .take()
            .ok_or_else(|| Err("value without key".into()))?;
        self.section(&key, value)
    }

    fn end(self) -> Result<(), Err> {
        Ok(())
    }
}

/// Serializes a section: a struct or map of keys. Returns `false` if the section is `None`.
struct SectionSerializer<'a> {
    output: &'a mut String,
}

impl<'a> ser::Serializer for SectionSerializer<'a> {
    type Ok = bool;
    type Error = Err;
    type SerializeSeq = ser::Impossible<bool, Err>;
    type SerializeTuple = ser::Impossible<bool, Err>;
    type SerializeTupleStruct = ser::Impossible<bool, Err>;
    type SerializeTupleVariant = ser::Impossible<bool, Err>;
    type SerializeMap = KeysSerializer<'a>;
    type SerializeStruct = KeysSerializer<'a>;
    type SerializeStructVariant = ser::Impossible<bool, Err>;

    unsupported_scalars!("section");
    unsupported! { "section":
        serialize_seq(len: Option<usize>) -> Self::SerializeSeq;
    }

    fn serialize_none(self) -> Result<bool, Err> {
        Ok(false)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<bool, Err> {
        value.serialize(self)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<bool, Err> {
        value.serialize(self)
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Err> {
        Ok(KeysSerializer {
            output: self.output,
            key: None,
        })
    }

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Err> {
        self.serialize_map(None)
    }
}

struct KeysSerializer<'a> {
    output: &'a mut String,
    key: Option<String>,
}

impl KeysSerializer<'_> {
    fn entry<T: ?Sized + Serialize>(&mut self, key: &str, value: &T) -> Result<(), Err> {
        let mut values = vec![];
        value.serialize(ValuesSerializer {
            values: &mut values,
        })?;
        for value in values {
            self.output.push_str(key);
            self.output.push('=');
            self.output.push_str(&value);
            self.output.push('\n');
        }
        Ok(())
    }
}

impl ser::SerializeStruct for KeysSerializer<'_> {
    type Ok = bool;
    type Error = Err;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Err> {
        self.entry(key, value)
    }

    fn end(self) -> Result<bool, Err> {
        Ok(true)
    }
}

impl ser::SerializeMap for KeysSerializer<'_> {
    type Ok = bool;
    type Error = Err;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Err> {
        self.key = Some(scalar(key)?);
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Err> {
        let key = self
            .key
            .take()
            .ok_or_else(|| Err("value without key".into()))?;
        self.entry(&key, value)
    }

    fn end(self) -> Result<bool, Err> {
        Ok(true)
    }
}

/// Serialize a single scalar into a string.
fn scalar<T: ?Sized + Serialize>(value: &T) -> Result<String, Err> {
    let mut values = vec![];
    value.serialize(ScalarSerializer {
        values: &mut values,
    })?;
    values
        .pop()
        .ok_or_else(|| Err("expected a value, got none".into()))
}

/// Serializes a value into zero (`None`), one (scalar) or more (sequence) strings.
struct ValuesSerializer<'a> {
    values: &'a mut Vec<String>,
}

/// Serializes a scalar into a string, pushing it onto `values`.
struct ScalarSerializer<'a> {
    values: &'a mut Vec<String>,
}

impl ScalarSerializer<'_> {
    fn push(self, value: String) -> Result<(), Err> {
        if value.contains(['\n', '\r']) {
            return Result::Err(Err(format!("value {value:?} contains a newline")));
        }
        self.values.push(value);
        Ok(())
    }
}

/// Implement the scalar `serialize_*` methods of [`ser::Serializer`] using [`ToString`].
macro_rules! to_string_scalars {
    ($($method:ident($ty:ty);)*) => {$(
        fn $method(self, v: $ty) -> Result<(), Err> {
            self.push(v.to_string())
        }
    )*};
}

impl ser::Serializer for ScalarSerializer<'_> {
    type Ok = ();
    type Error = Err;
    type SerializeSeq = ser::Impossible<(), Err>;
    type SerializeTuple = ser::Impossible<(), Err>;
    type SerializeTupleStruct = ser::Impossible<(), Err>;
    type SerializeTupleVariant = ser::Impossible<(), Err>;
    type SerializeMap = ser::Impossible<(), Err>;
    type SerializeStruct = ser::Impossible<(), Err>;
    type SerializeStructVariant = ser::Impossible<(), Err>;

    to_string_scalars! {
        serialize_bool(bool);
        serialize_i8(i8);
        serialize_i16(i16);
        serialize_i32(i32);
        serialize_i64(i64);
        serialize_u8(u8);
        serialize_u16(u16);
        serialize_u32(u32);
        serialize_u64(u64);
        serialize_f32(f32);
        serialize_f64(f64);
        serialize_char(char);
        serialize_str(&str);
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<(), Err> {
        self.push(variant.to_owned())
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Err> {
        value.serialize(self)
    }

    unsupported! { "value":
        serialize_bytes(v: &[u8]) -> Self::Ok;
        serialize_none() -> Self::Ok;
        serialize_unit() -> Self::Ok;
        serialize_unit_struct(name: &'static str) -> Self::Ok;
        serialize_seq(len: Option<usize>) -> Self::SerializeSeq;
        serialize_tuple(len: usize) -> Self::SerializeTuple;
        serialize_tuple_struct(name: &'static str, len: usize) -> Self::SerializeTupleStruct;
        serialize_tuple_variant(name: &'static str, idx: u32, variant: &'static str, len: usize) -> Self::SerializeTupleVariant;
        serialize_map(len: Option<usize>) -> Self::SerializeMap;
        serialize_struct(name: &'static str, len: usize) -> Self::SerializeStruct;
        serialize_struct_variant(name: &'static str, idx: u32, variant: &'static str, len: usize) -> Self::SerializeStructVariant;
    }

    fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<(), Err> {
        unsupported("serialize_some", "value")
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<(), Err> {
        unsupported("serialize_newtype_variant", "value")
    }
}

/// Forward scalar `serialize_*` methods of [`ValuesSerializer`] to [`ScalarSerializer`].
macro_rules! forward_scalars {
    ($($method:ident($($arg:ident: $ty:ty),*);)*) => {$(
        fn $method(self, $($arg: $ty),*) -> Result<(), Err> {
            ser::Serializer::$method(ScalarSerializer { values: self.values }, $($arg),*)
        }
    )*};
}

impl ser::Serializer for ValuesSerializer<'_> {
    type Ok = ();
    type Error = Err;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = ser::Impossible<(), Err>;
    type SerializeTupleVariant = ser::Impossible<(), Err>;
    type SerializeMap = ser::Impossible<(), Err>;
    type SerializeStruct = ser::Impossible<(), Err>;
    type SerializeStructVariant = ser::Impossible<(), Err>;

    forward_scalars! {
        serialize_bool(v: bool);
        serialize_i8(v: i8);
        serialize_i16(v: i16);
        serialize_i32(v: i32);
        serialize_i64(v: i64);
        serialize_u8(v: u8);
        serialize_u16(v: u16);
        serialize_u32(v: u32);
        serialize_u64(v: u64);
        serialize_f32(v: f32);
        serialize_f64(v: f64);
        serialize_char(v: char);
        serialize_str(v: &str);
        serialize_unit_variant(name: &'static str, idx: u32, variant: &'static str);
    }

    fn serialize_none(self) -> Result<(), Err> {
        Ok(())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Err> {
        value.serialize(self)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Err> {
        value.serialize(self)
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self, Err> {
        Ok(self)
    }

    fn serialize_tuple(self, _: usize) -> Result<Self, Err> {
        Ok(self)
    }

    unsupported! { "value":
        serialize_bytes(v: &[u8]) -> Self::Ok;
        serialize_unit() -> Self::Ok;
        serialize_unit_struct(name: &'static str) -> Self::Ok;
        serialize_tuple_struct(name: &'static str, len: usize) -> Self::SerializeTupleStruct;
        serialize_tuple_variant(name: &'static str, idx: u32, variant: &'static str, len: usize) -> Self::SerializeTupleVariant;
        serialize_map(len: Option<usize>) -> Self::SerializeMap;
        serialize_struct(name: &'static str, len: usize) -> Self::SerializeStruct;
        serialize_struct_variant(name: &'static str, idx: u32, variant: &'static str, len: usize) -> Self::SerializeStructVariant;
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<(), Err> {
        unsupported("serialize_newtype_variant", "value")
    }
}

impl ser::SerializeSeq for ValuesSerializer<'_> {
    type Ok = ();
    type Error = Err;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Err> {
        value.serialize(ScalarSerializer {
            values: self.values,
        })
    }

    fn end(self) -> Result<(), Err> {
        Ok(())
    }
}

impl ser::SerializeTuple for ValuesSerializer<'_> {
    type Ok = ();
    type Error = Err;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Err> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<(), Err> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "PascalCase")]
    struct Unit {
        partition: Partition,
        install: Option<BTreeMap<String, String>>,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    enum Format {
        #[serde(rename = "btrfs")]
        Btrfs,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "PascalCase")]
    struct Partition {
        label: String,
        weight: u32,
        factory_reset: bool,
        format: Format,
        mount_point: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        copy_blocks: Option<String>,
    }

    fn unit() -> Unit {
        Unit {
            partition: Partition {
                label: "My \"Label\"".into(),
                weight: 42,
                factory_reset: true,
                format: Format::Btrfs,
                mount_point: vec!["/:subvol=/".into(), "/home:subvol=/home".into()],
                copy_blocks: None,
            },
            install: Some(BTreeMap::from([(
                "WantedBy".into(),
                "multi-user.target".into(),
            )])),
        }
    }

    #[test]
    fn test_to_string() {
        assert_eq!(
            super::to_string(&unit()).unwrap(),
            "[Partition]
Label=My \"Label\"
Weight=42
FactoryReset=true
Format=btrfs
MountPoint=/:subvol=/
MountPoint=/home:subvol=/home

[Install]
WantedBy=multi-user.target
"
        );
    }

    #[test]
    fn test_round_trip() {
        let unit = unit();
        let s = super::to_string(&unit).unwrap();
        assert_eq!(crate::from_str::<Unit>(&s).unwrap(), unit);
    }

    #[test]
    fn test_none_section_is_skipped() {
        let mut unit = unit();
        unit.install = None;
        assert!(!super::to_string(&unit).unwrap().contains("[Install]"));
    }

    #[test]
    fn test_errors() {
        let mut unit = unit();
        unit.partition.label = "two\nlines".into();
        super::to_string(&unit).unwrap_err();
        super::to_string(&"not a unit").unwrap_err();
        super::to_string(&BTreeMap::from([("Section", "not a section")])).unwrap_err();
    }
}
