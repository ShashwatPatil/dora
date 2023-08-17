use arrow::{
    array::{
        make_array, ArrayData, BooleanBuilder, Float32Builder, Float64Builder, Int64Builder,
        NullArray, StringBuilder, StructArray, UInt64Builder,
    },
    datatypes::{DataType, Fields},
};
use core::fmt;
use std::ops::Deref;

pub struct Ros2Value(ArrayData);

impl Deref for Ros2Value {
    type Target = ArrayData;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedDeserializer {
    type_info: DataType,
}

impl TypedDeserializer {
    pub fn new(type_info: DataType) -> Self {
        Self { type_info }
    }
}

impl<'de> serde::de::DeserializeSeed<'de> for TypedDeserializer {
    type Value = Ros2Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = match self.type_info {
            DataType::Struct(fields) => {
                /// Serde requires that struct and field names are known at
                /// compile time with a `'static` lifetime, which is not
                /// possible in this case. Thus, we need to use dummy names
                /// instead.
                ///
                /// The actual names do not really matter because
                /// the CDR format of ROS2 does not encode struct or field
                /// names.
                const DUMMY_STRUCT_NAME: &str = "struct";
                const DUMMY_FIELDS: &[&str] = &[""; 100];

                deserializer.deserialize_struct(
                    DUMMY_STRUCT_NAME,
                    &DUMMY_FIELDS[..fields.len()],
                    StructVisitor { fields },
                )
            }
            DataType::Int32 => deserializer.deserialize_i32(PrimitiveValueVisitor),
            DataType::Float32 => deserializer.deserialize_f32(PrimitiveValueVisitor),
            DataType::Float64 => deserializer.deserialize_f64(PrimitiveValueVisitor),
            DataType::Utf8 => deserializer.deserialize_str(PrimitiveValueVisitor),
            _ => todo!(),
        }?;
        Ok(Ros2Value(value))
    }
}

/// Based on https://docs.rs/serde_yaml/0.9.22/src/serde_yaml/value/de.rs.html#14-121
struct PrimitiveValueVisitor;

impl<'de> serde::de::Visitor<'de> for PrimitiveValueVisitor {
    type Value = ArrayData;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a primitive value")
    }

    fn visit_bool<E>(self, b: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut array = BooleanBuilder::new();
        array.append_value(b);
        Ok(array.finish().into())
    }

    fn visit_i64<E>(self, i: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut array = Int64Builder::new();
        array.append_value(i);
        Ok(array.finish().into())
    }

    fn visit_u64<E>(self, u: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut array = UInt64Builder::new();
        array.append_value(u);
        Ok(array.finish().into())
    }

    fn visit_f32<E>(self, f: f32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut array = Float32Builder::new();
        array.append_value(f);
        Ok(array.finish().into())
    }

    fn visit_f64<E>(self, f: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut array = Float64Builder::new();
        array.append_value(f);
        Ok(array.finish().into())
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut array = StringBuilder::new();
        array.append_value(s);
        Ok(array.finish().into())
    }

    fn visit_string<E>(self, s: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut array = StringBuilder::new();
        array.append_value(s);
        Ok(array.finish().into())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let array = NullArray::new(0);
        Ok(array.into())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let array = NullArray::new(0);
        Ok(array.into())
    }
}

struct StructVisitor {
    fields: Fields,
}

impl<'de> serde::de::Visitor<'de> for StructVisitor {
    type Value = ArrayData;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a struct encoded as sequence")
    }

    fn visit_seq<A>(self, mut data: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut fields = vec![];
        for field in self.fields.iter() {
            let value = match data.next_element_seed(TypedDeserializer {
                type_info: field.data_type().clone(),
            })? {
                Some(value) => value.0,
                    None => {
                        return Err(serde::de::Error::custom(format!(
                            "missing field {}",
                        &field.name()
                        )))
                    }
            };
            fields.push((field.clone(), make_array(value)));
        }

        let struct_array: StructArray = fields.into();
        Ok(struct_array.into())
    }
}
