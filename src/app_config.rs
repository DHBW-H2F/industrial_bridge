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

/// Defines all supported device configurations for the application.
///
/// # Fields
/// - `modbus_tcp`: Optional collection of Modbus TCP devices, keyed by name.
/// - `modbus_rtu`: Optional collection of Modbus RTU devices, keyed by name.
/// - `s7`: Optional collection of Siemens S7 PLC devices, keyed by name.
///
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

/// Defines all remote backends where collected measurements can be sent.
///
/// Each remote integration is optional and organized in maps keyed by a
/// user-defined name. This allows configuring multiple instances of the
/// same remote type (e.g., two InfluxDB databases).
///
/// # Fields
/// - `influx_db`: Optional collection of InfluxDB remotes, keyed by name.
/// - `prometheus`: Optional collection of Prometheus push remotes, keyed by name.
pub struct Remotes {
    #[device(Client)]
    pub influx_db: Option<HashMap<String, InfluxDBRemote>>,
    #[device(PrometheusMetricsPusher)]
    pub prometheus: Option<HashMap<String, PrometheusRemote>>,
}

#[derive(Deserialize, Debug)]
/// Global application configuration.
///
/// This is the top-level configuration structure combining devices,
/// remotes, and runtime parameters. It is typically deserialized from
/// a configuration file (e.g., TOML or YAML).
///
/// # Fields
/// - `devices`: All configured PLCs and field devices (`Devices`).
/// - `remotes`: All configured remote data sinks (`Remotes`).
/// - `period`: Collection period in milliseconds or seconds (depending on implementation).
/// - `timeout`: Optional timeout (in milliseconds) for communication requests.
pub struct AppConfig {
    pub devices: Devices,
    pub remotes: Remotes,
    pub period: u64,
    pub timeout: Option<u64>,
}
