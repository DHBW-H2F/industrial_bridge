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
