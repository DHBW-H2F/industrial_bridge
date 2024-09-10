use std::collections::HashMap;

use influxdb::Client;
use prometheus_push::prometheus_crate::PrometheusMetricsPusher;
use serde::Deserialize;

use crate::devices::modbus_rtu::ModbusRTUDevice;
use crate::devices::modbus_tcp::ModbusTCPDevice;
use crate::remotes::influxdb::InfluxDBRemote;
use crate::remotes::prometheus::PrometheusRemote;

use macros::IntoHashMap;

use crate::remotes::remote::Remote;
use industrial_device::IndustrialDevice;

use crate::devices::errors::DeviceInitError;
use crate::remotes::errors::RemoteInitError;

use modbus_device::ModbusDeviceAsync;
use s7_device::S7Device;

#[derive(Deserialize, Debug, IntoHashMap)]
#[implementation(IndustrialDevice, DeviceInitError)]
pub struct Devices {
    #[device(ModbusDeviceAsync)]
    pub modbus_tcp: Option<HashMap<String, ModbusTCPDevice>>,
    #[device(ModbusDeviceAsync)]
    pub modbus_rtu: Option<HashMap<String, ModbusRTUDevice>>,
    #[device(S7Device)]
    pub s7: Option<HashMap<String, crate::devices::s7::S7Device>>,
}

#[derive(Deserialize, Debug, IntoHashMap)]
#[implementation(Remote, RemoteInitError)]
pub struct Remotes {
    #[device(Client)]
    pub influx_db: Option<HashMap<String, InfluxDBRemote>>,
    #[device(PrometheusMetricsPusher)]
    pub prometheus: Option<HashMap<String, PrometheusRemote>>,
}

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    pub devices: Devices,
    pub remotes: Remotes,
    pub period: u64,
    pub timeout: Option<u64>,
}
