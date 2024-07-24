use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct ModbusTCPDevice {
    pub remote: String,
    pub input_registers: String,
    pub holding_registers: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct ModbusRTUDevice {
    pub port: String,
    pub slave: u32,
    pub speed: u32,
    pub input_registers: String,
    pub holding_registers: String,
}

#[derive(Deserialize, Debug, Clone)]
pub enum ModbusDevice {
    TCP(ModbusTCPDevice),
    RTU(ModbusRTUDevice),
}
impl Into<ModbusDevice> for ModbusRTUDevice {
    fn into(self) -> ModbusDevice {
        ModbusDevice::RTU(self)
    }
}
impl Into<ModbusDevice> for ModbusTCPDevice {
    fn into(self) -> ModbusDevice {
        ModbusDevice::TCP(self)
    }
}

#[derive(Deserialize, Debug)]
pub struct S7Devices {
    pub remote: String,
    pub registers: String,
}

#[derive(Deserialize, Debug)]
pub struct ModbusDevices {
    pub tcp: Option<HashMap<String, ModbusTCPDevice>>,
    pub rtu: Option<HashMap<String, ModbusRTUDevice>>,
}

#[derive(Deserialize, Debug)]
pub struct Devices {
    pub modbus: Option<ModbusDevices>,
    pub s7: Option<HashMap<String, S7Devices>>,
}

#[derive(Deserialize, Debug)]
pub struct InfluxDBRemote {
    pub remote: String,
    pub bucket: String,
    pub token: String,
}

#[derive(Deserialize, Debug)]
pub struct PrometheusRemote {
    pub remote: String,
}

#[derive(Deserialize, Debug)]
pub struct Remotes {
    pub influx_db: Option<HashMap<String, InfluxDBRemote>>,
    pub prometheus: Option<HashMap<String, PrometheusRemote>>,
}

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    pub devices: Devices,
    pub remotes: Remotes,
    pub period: u64,
}
