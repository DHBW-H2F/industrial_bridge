use std::collections::HashMap;

use serde::Deserialize;

use crate::devices::modbus_rtu::ModbusRTUDevice;
use crate::devices::modbus_tcp::ModbusTCPDevice;

use macros::IntoHashMap;

use industrial_device::IndustrialDevice;

use crate::devices::errors::DeviceInitError;

use modbus_device::ModbusDeviceAsync;
use s7_device::S7Device;

#[derive(Deserialize, Debug, IntoHashMap)]
pub struct Devices {
    #[device(ModbusDeviceAsync)]
    pub modbus_tcp: Option<HashMap<String, ModbusTCPDevice>>,
    #[device(ModbusDeviceAsync)]
    pub modbus_rtu: Option<HashMap<String, ModbusRTUDevice>>,
    #[device(S7Device)]
    pub s7: Option<HashMap<String, crate::devices::s7::S7Device>>,
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
    pub timeout: Option<u64>,
}
