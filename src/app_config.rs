use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ModbusDevices {
    pub remote: String,
    pub input_registers: String,
    pub holding_registers: String,
}

#[derive(Deserialize, Debug)]
pub struct S7Devices {
    remote: String,
    registers: String,
}

#[derive(Deserialize, Debug)]
pub struct Devices {
    pub modbus: HashMap<String, ModbusDevices>,
    s7: HashMap<String, S7Devices>,
}

#[derive(Deserialize, Debug)]
pub struct InfluxDBRemote {
    remote: String,
    bucket: String,
    token: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PrometheusRemote {
    remote: String,
}

#[derive(Deserialize, Debug)]
pub struct Remotes {
    influx_db: HashMap<String, InfluxDBRemote>,
    prometheus: HashMap<String, PrometheusRemote>,
}

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    pub devices: Devices,
    pub remotes: Remotes,
    pub period: u64,
}
