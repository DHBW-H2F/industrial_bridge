use std::{collections::HashMap, hash::Hash};

use influxdb::Type;

#[derive(Debug, Clone)]
pub enum RegisterValue {
    Modbus(modbus_device::types::RegisterValue),
    S7(s7_device::types::RegisterValue),
}

impl From<s7_device::types::RegisterValue> for RegisterValue {
    fn from(value: s7_device::types::RegisterValue) -> Self {
        RegisterValue::S7(value)
    }
}
impl From<modbus_device::types::RegisterValue> for RegisterValue {
    fn from(value: modbus_device::types::RegisterValue) -> Self {
        RegisterValue::Modbus(value)
    }
}

// Ugly conversion because of https://github.com/rust-lang/rust/issues/31844
pub fn convert_hashmap<K: Hash + Eq + Clone, V1: Into<V2> + Clone, V2>(
    val: HashMap<K, V1>,
) -> HashMap<K, V2> {
    val.iter()
        .map(|(name, field)| ((*name).clone(), (*field).clone().into()))
        .collect()
}

impl Into<Type> for RegisterValue {
    fn into(self) -> Type {
        match self {
            RegisterValue::Modbus(val) => match val {
                modbus_device::types::RegisterValue::U16(val) => val.into(),
                modbus_device::types::RegisterValue::U32(val) => val.into(),
                modbus_device::types::RegisterValue::U64(val) => val.into(),
                modbus_device::types::RegisterValue::U128(val) => val.to_string().into(),
                modbus_device::types::RegisterValue::S32(val) => val.into(),
                modbus_device::types::RegisterValue::Enum16(val) => val.into(),
                modbus_device::types::RegisterValue::Sized(val) => format!("{0:x?}", &val).into(),
                modbus_device::types::RegisterValue::Float32(val) => match val.is_nan() {
                    true => (-1.0).into(),
                    _ => val.into(),
                },
                modbus_device::types::RegisterValue::Boolean(val) => val.into(),
            },
            RegisterValue::S7(val) => match val {
                s7_device::types::RegisterValue::S16(val) => val.into(),
                s7_device::types::RegisterValue::S32(val) => val.into(),
                s7_device::types::RegisterValue::Float32(val) => match val.is_nan() {
                    true => (1.0).into(),
                    _ => val.into(),
                },
                s7_device::types::RegisterValue::Boolean(val) => val.into(),
            },
        }
    }
}

impl Into<String> for RegisterValue {
    fn into(self) -> String {
        match self {
            RegisterValue::Modbus(val) => match val {
                modbus_device::types::RegisterValue::U16(val) => val.to_string(),
                modbus_device::types::RegisterValue::U32(val) => val.to_string(),
                modbus_device::types::RegisterValue::U64(val) => val.to_string(),
                modbus_device::types::RegisterValue::U128(val) => val.to_string(),
                modbus_device::types::RegisterValue::S32(val) => val.to_string(),
                modbus_device::types::RegisterValue::Enum16(val) => val.to_string(),
                modbus_device::types::RegisterValue::Sized(val) => "Not implemented".to_string(),
                modbus_device::types::RegisterValue::Float32(val) => val.to_string(),
                modbus_device::types::RegisterValue::Boolean(val) => match val {
                    true => "1".to_string(),
                    false => "2".to_string(),
                },
            },
            RegisterValue::S7(_) => todo!(),
        }
    }
}

impl Into<f64> for RegisterValue {
    fn into(self) -> f64 {
        match self {
            RegisterValue::Modbus(val) => match val {
                modbus_device::types::RegisterValue::U16(val) => val.into(),
                modbus_device::types::RegisterValue::U32(val) => val.into(),
                modbus_device::types::RegisterValue::U64(val) => val as f64,
                modbus_device::types::RegisterValue::U128(val) => val as f64,
                modbus_device::types::RegisterValue::S32(val) => val.into(),
                modbus_device::types::RegisterValue::Enum16(val) => val.into(),
                modbus_device::types::RegisterValue::Sized(_) => 0 as f64,
                modbus_device::types::RegisterValue::Float32(val) => match val.is_nan() {
                    true => (-1.0).into(),
                    _ => val.into(),
                },
                modbus_device::types::RegisterValue::Boolean(val) => val.into(),
            },
            RegisterValue::S7(val) => match val {
                s7_device::types::RegisterValue::S16(val) => val.into(),
                s7_device::types::RegisterValue::S32(val) => val.into(),
                s7_device::types::RegisterValue::Float32(val) => val.into(),
                s7_device::types::RegisterValue::Boolean(val) => val.into(),
            },
        }
    }
}
