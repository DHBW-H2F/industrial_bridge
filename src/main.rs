use core::panic;
use devices::{connect_devices, fetch_device};
use influxdb::{InfluxDbWriteable, Type};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, fs::File};
use tokio::join;
use tokio_modbus::Slave;

use log::{debug, error, info};

use clap::Parser;

use modbus_device::types::{ModBusContext, RTUContext, TCPContext};
use tokio::sync::{Mutex, Notify, RwLock};

use prometheus::Gauge;
use url::Url;

use prometheus_push::prometheus_crate::PrometheusMetricsPusher;

use modbus_device;
use modbus_device::modbus_device_async::ModbusDeviceAsync;
use modbus_device::utils::get_defs_from_json;

use s7_device::S7Device;

use config;

mod app_config;
use app_config::{AppConfig, ModbusDevice};

mod types_conversion;
use types_conversion::RegisterValue;

mod devices;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long,
        default_value = "config.yaml",
        help = "Config path",
        long_help = "Where to find the config file"
    )]
    config_file: String,
}

struct Devices {
    modbus: Rc<RefCell<HashMap<String, Arc<Mutex<ModbusDeviceAsync>>>>>,
    s7: Rc<RefCell<HashMap<String, Arc<Mutex<S7Device>>>>>,
}

// #[derive(Debug)]
struct Remotes {
    influxdb: Arc<Mutex<HashMap<String, Arc<Mutex<influxdb::Client>>>>>,
    prometheus: Arc<Mutex<HashMap<String, Arc<Mutex<PrometheusMetricsPusher>>>>>,
}

async fn send_data_to_remotes(
    remotes: Arc<Mutex<Remotes>>,
    data_available: Arc<Notify>,
    data: Arc<RwLock<HashMap<String, HashMap<String, RegisterValue>>>>,
) {
    loop {
        // wait for new data
        let _ = data_available.notified().await;

        info!("New data available : starting push");

        let remotes = remotes.lock().await;
        join!(
            send_data_to_influxdb(remotes.influxdb.clone(), data.clone()),
            send_data_to_prometheus(remotes.prometheus.clone(), data.clone()),
        );
    }
}

async fn send_data_to_prometheus(
    remotes: Arc<Mutex<HashMap<String, Arc<Mutex<PrometheusMetricsPusher>>>>>,
    data: Arc<RwLock<HashMap<String, HashMap<String, RegisterValue>>>>,
) {
    for (name, remote) in remotes.lock().await.iter() {
        info!("Sending to remote {name}");
        for (source, values) in data.read().await.iter() {
            let registry = prometheus::Registry::new();
            for (field, value) in values {
                let gauge =
                    Gauge::new(field.replace(&['-', '/', '[', ']', '%'][..], "_"), field).unwrap();
                gauge.set(value.clone().into());
                registry.register(Box::new(gauge)).unwrap();
            }

            match remote
                .lock()
                .await
                .push_all(source, &HashMap::new(), registry.gather())
                .await
            {
                Ok(_) => {}
                Err(err) => error!("There was an error sending data to remote {name} ({err:?})"),
            };
        }
    }
}

async fn send_data_to_influxdb(
    remotes: Arc<Mutex<HashMap<String, Arc<Mutex<influxdb::Client>>>>>,
    data: Arc<RwLock<HashMap<String, HashMap<String, RegisterValue>>>>,
) {
    for (name, remote) in remotes.lock().await.iter() {
        info!("Sending to remote {name}");
        for (source, values) in data.read().await.iter() {
            let mut query =
                influxdb::Timestamp::from(chrono::offset::Local::now()).into_query(source);
            for (field, value) in values {
                query = query.add_field(field, Into::<Type>::into(value.clone()));
            }

            match remote.lock().await.query(query).await {
                Ok(res) => {
                    if !res.is_empty() {
                        error!("There was an error sending data to remote {name} ({res})");
                    }
                }
                Err(err) => error!("There was an error sending data to remote {name} ({err})"),
            };
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize utils
    env_logger::init();
    let args = Args::parse();
    let config = config::Config::builder()
        .add_source(config::File::with_name(&args.config_file))
        .build()
        .unwrap();

    let app: AppConfig = config.try_deserialize().unwrap();

    // Initialize our targets from config
    // panic on error (better catch it here at launch)
    let devices = Devices {
        modbus: Rc::new(RefCell::new(HashMap::new())),
        s7: Rc::new(RefCell::new(HashMap::new())),
    };

    let modbus_devices = match app.devices.modbus {
        None => {
            info!("There is no modbus devices set ");
            HashMap::new()
        }
        Some(modbus_devices) => {
            let mut res_devices: HashMap<String, app_config::ModbusDevice> =
                match modbus_devices.tcp {
                    Some(tcp_devices) => tcp_devices
                        .iter()
                        .map(|(val, field)| (val.clone(), field.clone().into()))
                        .collect(),
                    None => {
                        info!("There is no modbus TCP devices set");
                        HashMap::new()
                    }
                };
            match modbus_devices.rtu {
                Some(rtu_devices) => {
                    res_devices.extend(
                        rtu_devices
                            .iter()
                            .map(|(val, field)| (val.clone(), field.clone().into()))
                            .collect::<HashMap<String, app_config::ModbusDevice>>(),
                    );
                }
                None => info!("There is no modbus RTU devices set"),
            }
            res_devices
        }
    };

    for (name, device) in &modbus_devices {
        let device_ctx: ModBusContext = match device {
            app_config::ModbusDevice::TCP(dev) => {
                let addr = match dev.remote.parse() {
                    Ok(addr) => addr,
                    Err(err) => panic!("Invalid remote address entered {0} ({err})", dev.remote),
                };
                TCPContext { addr }.into()
            }
            app_config::ModbusDevice::RTU(dev) => RTUContext {
                port: dev.port.clone(),
                slave: Slave(dev.slave as u8),
                speed: dev.speed,
            }
            .into(),
        };
        let input_registers = match device {
            ModbusDevice::TCP(dev) => &dev.input_registers,
            ModbusDevice::RTU(dev) => &dev.input_registers,
        };
        let holding_registers = match device {
            ModbusDevice::TCP(dev) => &dev.holding_registers,
            ModbusDevice::RTU(dev) => &dev.holding_registers,
        };
        let input_registers_json = match File::open(input_registers.clone()) {
            Ok(file) => file,
            Err(err) => panic!(
                "Could not open the file containing the input registers definition : {0} ({err:?})",
                input_registers
            ),
        };
        let holding_registers_json = match File::open(holding_registers.clone()) {
            Ok(file) => file,
            Err(err) => panic!(
                "Could not open the file containing the holding registers definition : {0} ({err:?})",
                holding_registers
            ),
        };
        let d = ModbusDeviceAsync::new(
            device_ctx,
            match get_defs_from_json(input_registers_json) {
                Ok(registers) => registers,
                Err(err) => panic!("Could not load input registers definition from file ({err})"),
            },
            match get_defs_from_json(holding_registers_json) {
                Ok(registers) => registers,
                Err(err) => panic!("Could not load holding registers definition from file ({err})"),
            },
        );
        devices
            .modbus
            .clone()
            .borrow_mut()
            .insert(name.clone(), Arc::new(Mutex::new(d)));
    }
    let s7_devices = match app.devices.s7 {
        None => {
            info!("There is no s7 devices set ");
            HashMap::new()
        }
        Some(s7_devices) => s7_devices,
    };

    for (name, device) in &s7_devices {
        let registers_json = match File::open(device.registers.clone()) {
            Ok(file) => file,
            Err(err) => panic!(
                "Could not open the file containing the registers definition : {0} ({err:?})",
                device.registers
            ),
        };
        let d = S7Device::new(
            device.remote.parse().unwrap(),
            match s7_device::utils::get_defs_from_json(registers_json) {
                Ok(registers) => registers,
                Err(err) => panic!("Could not load registers definition from file ({err:?})"),
            },
        );
        devices
            .s7
            .clone()
            .borrow_mut()
            .insert(name.clone(), Arc::new(Mutex::new(d)));
    }

    // Initialize the remotes
    let remotes = Remotes {
        influxdb: Arc::new(Mutex::new(HashMap::new())),
        prometheus: Arc::new(Mutex::new(HashMap::new())),
    };

    match app.remotes.influx_db {
        None => info!("There is no influxdb remote"),
        Some(influxdb_remotes) => {
            for (name, remote) in influxdb_remotes {
                let remote_dev = Arc::new(Mutex::new(
                    influxdb::Client::new(remote.remote.clone(), remote.bucket.clone())
                        .with_token(remote.token.clone()),
                ));
                match remote_dev.lock().await.ping().await {
                    Ok(res) => info!("Succesfully connected to {name} ({res:?})"),
                    Err(err) => panic!("Could not connect to remote {name} ({err})"),
                };
                remotes
                    .influxdb
                    .lock()
                    .await
                    .insert(name.clone(), remote_dev);
            }
        }
    }

    match app.remotes.prometheus {
        None => info!("There is no prometheus"),
        Some(prometheus_remotes) => {
            for (name, remote) in prometheus_remotes {
                let client = reqwest::Client::new();
                let pusher = Arc::new(Mutex::new(
                    PrometheusMetricsPusher::from(
                        client,
                        &Url::parse(remote.remote.as_str()).unwrap(),
                    )
                    .unwrap(),
                ));
                remotes.prometheus.lock().await.insert(name.clone(), pusher);
            }
        }
    }

    // connect to all modbus devices
    connect_devices(devices.modbus.clone()).await;
    // connect to all S7 devices
    connect_devices(devices.s7.clone()).await;

    // Data fetch is triggered at the interval entered in configuration
    let mut interval = tokio::time::interval(Duration::from_secs(app.period));

    let data_available = Arc::new(Notify::new());

    let data_received: Arc<RwLock<HashMap<String, HashMap<String, RegisterValue>>>> =
        Arc::new(RwLock::new(HashMap::new()));
    // Start the task that send data to remotes
    {
        let data_available = data_available.clone();
        let data_received = data_received.clone();
        tokio::task::spawn(async move {
            send_data_to_remotes(Arc::new(Mutex::new(remotes)), data_available, data_received)
                .await;
        });
    }

    loop {
        // Wait for the configured time
        interval.tick().await;

        // Fetch all data
        let data_out = data_received.clone();
        let mut rec_out = data_out.write().await;
        rec_out.clear();
        rec_out.extend(fetch_device(devices.modbus.clone()).await);

        rec_out.extend(fetch_device(devices.s7.clone()).await);
        debug!("{rec_out:?}");

        // Advertise the new data
        data_available.notify_one();
    }
}
