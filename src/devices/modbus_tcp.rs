use std::{fs::File, net::SocketAddr};

use modbus_device::{types::TCPContext, utils::get_defs_from_json, ModbusDeviceAsync};
use serde::Deserialize;

use super::errors::DeviceInitError;

#[derive(Deserialize, Debug, Clone)]
pub struct ModbusTCPDevice {
    pub remote: String,
    pub input_registers: String,
    pub holding_registers: String,
}

impl TryFrom<ModbusTCPDevice> for ModbusDeviceAsync {
    type Error = DeviceInitError;

    fn try_from(value: ModbusTCPDevice) -> Result<Self, Self::Error> {
        let input_registers_json = File::open(value.input_registers)?;
        let input_registers = get_defs_from_json(input_registers_json)?;

        let holding_registers_json = File::open(value.holding_registers)?;
        let holding_registers = get_defs_from_json(holding_registers_json)?;

        let addr: SocketAddr = value.remote.parse()?;
        let context = TCPContext { addr };

        Ok(ModbusDeviceAsync::new(
            context.into(),
            input_registers,
            holding_registers,
        ))
    }
}
