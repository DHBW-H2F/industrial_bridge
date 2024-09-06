use std::fs::File;

use modbus_device::{types::RTUContext, utils::get_defs_from_json, ModbusDeviceAsync};
use serde::Deserialize;
use tokio_modbus::Slave;

use super::errors::DeviceInitError;

#[derive(Deserialize, Debug, Clone)]
pub struct ModbusRTUDevice {
    pub port: String,
    pub slave: u32,
    pub speed: u32,
    pub input_registers: String,
    pub holding_registers: String,
}

impl TryFrom<ModbusRTUDevice> for ModbusDeviceAsync {
    type Error = DeviceInitError;

    fn try_from(value: ModbusRTUDevice) -> Result<Self, Self::Error> {
        let input_registers_json = File::open(value.input_registers)?;
        let input_registers = get_defs_from_json(input_registers_json)?;

        let holding_registers_json = File::open(value.holding_registers)?;
        let holding_registers = get_defs_from_json(holding_registers_json)?;

        let context = RTUContext {
            port: value.port,
            slave: Slave(value.slave as u8),
            speed: value.speed,
        };

        Ok(ModbusDeviceAsync::new(
            context.into(),
            input_registers,
            holding_registers,
        ))
    }
}
