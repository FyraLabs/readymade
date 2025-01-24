use serde::{ser, Serialize};

#[derive(Default)]
pub struct Serializer {
    output: String,
}

pub fn to_string<T: Serialize>(value: &T) -> Result<String, ()> {
    let mut serializer = Serializer::default();
    _ = value.serialize(&mut serializer);
    Ok(serializer.output)
}

#[derive(thiserror::Error, Debug)]
#[error("error")]
pub struct Err;
impl serde::ser::Error for Err {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Err
    }
}

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();
    type Error = Err;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.output += if v { "true" } else { "false" };
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.output += &v.to_string();
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(v))
    }
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(v))
    }
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(v))
    }
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.output += &v.to_string();
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(f64::from(v))
    }
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.output += &v.to_string();
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.output += "\"";
        self.output += &v.replace('"', "\\\"").replace('\n', "\\\n");
        self.output += "\"";
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.output += "[";
        self.output += name;
        self.output += "]\n";
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.output += "[";
        self.output += name;
        self.output += "]\n";
        value.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        todo!()
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        todo!()
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        todo!()
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        todo!()
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(self)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.output += "[";
        self.output += name;
        self.output += "]\n";
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.output += "\n";
        self.output += variant;
        self.output += "=";
        Ok(self)
    }
}

impl<'a> ser::SerializeSeq for &'a mut Serializer {
    type Ok = ();

    type Error = Err;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

impl<'a> ser::SerializeTuple for &'a mut Serializer {
    type Ok = ();

    type Error = Err;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    type Ok = ();

    type Error = Err;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    type Ok = ();

    type Error = Err;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

impl<'a> ser::SerializeMap for &'a mut Serializer {
    type Ok = ();

    type Error = Err;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

impl<'a> ser::SerializeStruct for &'a mut Serializer {
    type Ok = ();
    type Error = Err;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.output += key;
        self.output += "=";
        value.serialize(&mut **self)?;
        self.output += "\n";
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.output += "\n";
        Ok(())
    }
}

impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
    type Ok = ();

    type Error = Err;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    #[test]
    fn test_to_string() {
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct Meow {
            hello: String,
            bye: usize,
        }

        println!(
            "{}",
            super::to_string(&Meow {
                hello: "hai".into(),
                bye: 42,
            })
            .unwrap()
        );
    }
}
