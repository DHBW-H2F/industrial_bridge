use std::{fs::File, net::SocketAddr};

use s7_device::utils::{get_defs_from_json, JsonReadError};
use serde::Deserialize;

use super::errors::DeviceInitError;

#[derive(Deserialize, Debug, Clone)]
pub struct S7Device {
    pub remote: String,
    pub registers: String,
}

impl TryFrom<S7Device> for s7_device::S7Device {
    type Error = DeviceInitError;

    fn try_from(value: S7Device) -> Result<Self, Self::Error> {
        let registers_json = File::open(value.registers)?;
        let registers = get_defs_from_json(registers_json)?;

        let addr: SocketAddr = value.remote.parse()?;

        Ok(s7_device::S7Device::new(addr, registers))
    }
}

impl From<JsonReadError> for DeviceInitError {
    fn from(value: JsonReadError) -> Self {
        DeviceInitError::ParsingFailed {
            err: Box::new(value),
        }
    }
}
