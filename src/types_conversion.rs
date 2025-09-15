use std::{collections::HashMap, hash::Hash};

use industrial_device::types::Value;
use influxdb::Type;

#[derive(Debug, Clone)]
pub struct RegisterValue {
    value: Value,
}

impl From<Value> for RegisterValue {
    fn from(value: Value) -> Self {
        RegisterValue { value }
    }
}

/// Ugly conversion because of https://github.com/rust-lang/rust/issues/31844
/// Converts a `HashMap<K, V1>` into a `HashMap<K, V2>`
/// by transforming each value using the `Into` trait.
/// # Type Parameters
/// - `K`: the key type, must implement `Hash + Eq + Clone`.
/// - `V1`: the source value type, must implement `Clone` and `Into<V2>`.
/// - `V2`: the target value type.
///
/// # Parameters
/// - `val`: input `HashMap<K, V1>`
///
/// # Returns
/// A new `HashMap<K, V2>` with identical keys and converted values.
pub fn convert_hashmap<K: Hash + Eq + Clone, V1: Into<V2> + Clone, V2>(
    val: HashMap<K, V1>,
) -> HashMap<K, V2> {
    val.iter()
        .map(|(name, field)| ((*name).clone(), (*field).clone().into()))
        .collect()
}

impl Into<Type> for RegisterValue {
    fn into(self) -> Type {
        match self.value {
            Value::U16(val) => val.into(),
            Value::U32(val) => val.into(),
            Value::U64(val) => val.into(),
            Value::U128(val) => val.to_string().into(),
            Value::S16(val) => val.into(),
            Value::S32(val) => val.into(),
            Value::Enum16(val) => val.into(),
            Value::Sized(val) => format!("{0:x?}", &val).into(),
            Value::Float32(val) => match val.is_nan() {
                true => (-1.0).into(),
                _ => val.into(),
            },
            Value::Boolean(val) => val.into(),
        }
    }
}

impl Into<String> for RegisterValue {
    fn into(self) -> String {
        match self.value {
            Value::U16(val) => val.to_string(),
            Value::U32(val) => val.to_string(),
            Value::U64(val) => val.to_string(),
            Value::U128(val) => val.to_string(),
            Value::S16(val) => val.to_string(),
            Value::S32(val) => val.to_string(),
            Value::Enum16(val) => val.to_string(),
            Value::Sized(val) => format!("{0:x?}", &val),
            Value::Float32(val) => val.to_string(),
            Value::Boolean(val) => match val {
                true => "1".to_string(),
                false => "2".to_string(),
            },
        }
    }
}

impl Into<f64> for RegisterValue {
    fn into(self) -> f64 {
        match self.value {
            Value::U16(val) => val.into(),
            Value::U32(val) => val.into(),
            Value::U64(val) => val as f64,
            Value::U128(val) => val as f64,
            Value::S16(val) => val.into(),
            Value::S32(val) => val.into(),
            Value::Enum16(val) => val.into(),
            Value::Sized(_val) => 0 as f64,
            Value::Float32(val) => match val.is_nan() {
                true => (-1.0).into(),
                _ => val.into(),
            },
            Value::Boolean(val) => val.into(),
        }
    }
}
