use influxdb::Type;
use modbus_device::types::RegisterValue;

pub struct LocalRegisterValue(pub RegisterValue);

impl Into<Type> for LocalRegisterValue {
    fn into(self) -> Type {
        match self.0 {
            RegisterValue::U16(val) => val.into(),
            RegisterValue::U32(val) => val.into(),
            RegisterValue::U64(val) => val.into(),
            RegisterValue::U128(val) => val.to_string().into(),
            RegisterValue::S32(val) => val.into(),
            RegisterValue::Enum16(val) => val.into(),
            RegisterValue::Sized(val) => format!("{0:x?}", &val).into(),
            RegisterValue::Float32(val) => match val.is_nan() {
                true => (-1.0).into(),
                _ => val.into(),
            },
            RegisterValue::Boolean(val) => val.into(),
        }
    }
}

impl Into<String> for LocalRegisterValue {
    fn into(self) -> String {
        match self.0 {
            RegisterValue::U16(val) => val.to_string(),
            RegisterValue::U32(val) => val.to_string(),
            RegisterValue::U64(val) => val.to_string(),
            RegisterValue::U128(val) => val.to_string(),
            RegisterValue::S32(val) => val.to_string(),
            RegisterValue::Enum16(val) => val.to_string(),
            RegisterValue::Sized(val) => "Not implemented".to_string(),
            RegisterValue::Float32(val) => val.to_string(),
            RegisterValue::Boolean(val) => match val {
                true => "1".to_string(),
                false => "2".to_string(),
            },
        }
    }
}

impl Into<f64> for LocalRegisterValue {
    fn into(self) -> f64 {
        match self.0 {
            RegisterValue::U16(val) => val.into(),
            RegisterValue::U32(val) => val.into(),
            RegisterValue::U64(val) => val as f64,
            RegisterValue::U128(val) => val as f64,
            RegisterValue::S32(val) => val.into(),
            RegisterValue::Enum16(val) => val.into(),
            RegisterValue::Sized(_) => 0 as f64,
            RegisterValue::Float32(val) => match val.is_nan() {
                true => (-1.0).into(),
                _ => val.into(),
            },
            RegisterValue::Boolean(val) => val.into(),
        }
    }
}
