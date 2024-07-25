use core::panic;
use influxdb::{InfluxDbWriteable, Type};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, fs::File};
use tokio::join;
use tokio_modbus::Slave;

use log::{debug, error, info, warn};

use clap::Parser;

use modbus_device::errors::ModbusError;
use modbus_device::types::{ModBusContext, RTUContext, TCPContext};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio::task::JoinSet;

use backoff::ExponentialBackoff;

use prometheus::Gauge;
use url::Url;

use prometheus_push::prometheus_crate::PrometheusMetricsPusher;

mod app_config;
use app_config::{AppConfig, ModbusDevice};

mod types_conversion;
use types_conversion::{convert_hashmap, RegisterValue};

use modbus_device;
use modbus_device::modbus_device_async::ModbusConnexionAsync;
use modbus_device::modbus_device_async::ModbusDeviceAsync;
use modbus_device::utils::get_defs_from_json;

use s7_device::S7Connexion;
use s7_device::S7Device;

use config;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(
        short,
        long,
        default_value = "config.toml",
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

// Connect all devices passed as arguments to their targets, panics on error (this should then only be used in the initialisation)
// The connection for all devices is realized in parallel
async fn connect_modbus(
    modbus_devices: Rc<RefCell<HashMap<String, Arc<Mutex<ModbusDeviceAsync>>>>>,
) {
    // Create a task for each target
    let mut set = JoinSet::new();
    for (name, device) in modbus_devices.borrow().iter() {
        let d = device.clone();
        let name = name.clone();
        set.spawn(async move { (name, d.lock().await.connect().await) });
    }

    // Wait for completion
    async {
        while let Some(res) = set.join_next().await {
            match res {
                Ok((name, res)) => match res {
                    Ok(_) => info!("Connected to {name}"),
                    Err(err) => panic!("Could not connect to {name} ({err})"),
                },
                Err(err) => panic!("Error while joining connection threads ({err})"),
            }
        }
    }
    .await;
}
async fn connect_s7(s7_devices: Rc<RefCell<HashMap<String, Arc<Mutex<S7Device>>>>>) {
    // Create a task for each target
    let mut set = JoinSet::new();
    for (name, device) in s7_devices.borrow().iter() {
        let d = device.clone();
        let name = name.clone();
        set.spawn(async move { (name, d.lock().await.connect().await) });
    }

    // Wait for completion
    async {
        while let Some(res) = set.join_next().await {
            match res {
                Ok((name, res)) => match res {
                    Ok(_) => info!("Connected to {name}"),
                    Err(err) => panic!("Could not connect to {name} ({err:?})"),
                },
                Err(err) => panic!("Error while joining connection threads ({err})"),
            }
        }
    }
    .await;
}

// Manage errors occuring on a modbus data read, try to reconnect if a BrokenPipe is detected
async fn manage_modbus_error(
    err: ModbusError,
    device: Arc<Mutex<ModbusDeviceAsync>>,
) -> Result<(), ModbusError> {
    match err {
        ModbusError::ModbusError(tokio_modbus::Error::Transport(err)) => match err.kind() {
            std::io::ErrorKind::BrokenPipe => {
                error!("Broken pipe while reading register reconnecting to device ({err})");
                backoff::future::retry(ExponentialBackoff::default(), || async {
                    let connection_timeout_res =
                        tokio::time::timeout(Duration::from_secs(1), device.lock().await.connect())
                            .await;
                    let connection_res = match connection_timeout_res {
                        Ok(res) => res,
                        Err(err) => {
                            warn!("Connexion took too long, aborting ({err})");
                            return Err(backoff::Error::transient(()));
                        }
                    };
                    match connection_res {
                        Ok(_res) => {
                            info!("Reconnexion successful !");
                            Ok(())
                        }
                        Err(err) => {
                            warn!("Connexion error on reconnect, re-trying ({err})");
                            Err(backoff::Error::transient(()))
                        }
                    }
                })
                .await
                .unwrap();
                return Err(err.into());
            }
            _ => {
                error!("IOError reading registers, skipping this run ({err})");
                return Err(err.into());
            }
        },
        err => {
            error!("Error reading registers, skipping this run ({err:?})");
            return Err(err.into());
        }
    };
}

// For all the modbus devices passed, dump all registers and returns it as a HashMap<device_name, HashMap<register_name, register_value>>
// Calls manage_modbus_error on error to try to reconnect
// The data fetch if realized in parallel for each target
async fn fetch_modbus(
    modbus_devices: Rc<RefCell<HashMap<String, Arc<Mutex<ModbusDeviceAsync>>>>>,
) -> HashMap<String, HashMap<String, RegisterValue>> {
    // Create a task for each device
    let mut set = JoinSet::new();
    for (name, device) in modbus_devices.borrow().iter() {
        let d = device.clone();
        let name = name.clone();
        set.spawn(async move {
            info!("Fetching modbus input registers from {name}");
            let data: Result<HashMap<String, modbus_device::types::RegisterValue>, _> =
                d.lock().await.dump_input_registers().await;
            match data {
                Ok(val) => Some(HashMap::from([(name, convert_hashmap(val))])),
                Err(err) => {
                    let _ = manage_modbus_error(err, d.clone()).await;
                    return None;
                }
            }
        });
    }

    // join the tasks and merge the results
    let mut res: HashMap<String, HashMap<String, RegisterValue>> = HashMap::new();
    async {
        while let Some(result) = set.join_next().await {
            match result {
                Ok(val) => {
                    if val.is_some() {
                        res.extend(val.unwrap());
                    }
                }
                Err(err) => error!("There was an error joining the tasks responsible for fetching modbus data ({err})"),
            }
        }
    }
    .await;
    res
}
async fn fetch_s7(
    s7_devices: Rc<RefCell<HashMap<String, Arc<Mutex<S7Device>>>>>,
) -> HashMap<String, HashMap<String, RegisterValue>> {
    // Create a task for each device
    let mut set = JoinSet::new();
    for (name, device) in s7_devices.borrow().iter() {
        let d = device.clone();
        let name = name.clone();
        set.spawn(async move {
            info!("Fetching s7 registers from {name}");
            let data: Result<HashMap<String, s7_device::types::RegisterValue>, _> =
                d.lock().await.dump_registers().await;
            match data {
                Ok(val) => Some(HashMap::from([(name, convert_hashmap(val))])),
                Err(_err) => {
                    todo!();
                    // let _ = manage_s7_error(err, d.clone()).await;
                    // return None;
                }
            }
        });
    }

    // join the tasks and merge the results
    let mut res: HashMap<String, HashMap<String, RegisterValue>> = HashMap::new();
    async {
        while let Some(result) = set.join_next().await {
            match result {
                Ok(val) => {
                    if val.is_some() {
                        res.extend(val.unwrap())
                    }
                }
                Err(err) => error!(
                    "There was an error joining the tasks responsible for fetching s7 data ({err})"
                ),
            }
        }
    }
    .await;
    res
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
                    Gauge::new(field.replace(&['-', '/', '[', ']'][..], "_"), field).unwrap();
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
    connect_modbus(devices.modbus.clone()).await;
    // connect to all S7 devices
    connect_s7(devices.s7.clone()).await;

    // Data fetch is triggered at the interval entered in configuration
    let mut interval = tokio::time::interval(Duration::from_secs(app.period));

    let data_available = Arc::new(Notify::new());

    let data_received = Arc::new(RwLock::new(HashMap::new()));
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
        rec_out.extend(fetch_modbus(devices.modbus.clone()).await);

        rec_out.extend(fetch_s7(devices.s7.clone()).await);
        debug!("{rec_out:?}");

        // Advertise the new data
        data_available.notify_one();
    }
}
