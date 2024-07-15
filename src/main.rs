use core::panic;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;
use std::{collections::HashMap, fs::File};

use log::{debug, error, info, warn};

use clap::Parser;

use modbus_device::errors::ModbusError;
use modbus_device::types::RegisterValue;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use backoff::ExponentialBackoff;

mod app_config;
use app_config::AppConfig;

use modbus_device;
use modbus_device::modbus_device_async::ModbusConnexionAsync;
use modbus_device::modbus_device_async::ModbusDeviceAsync;
use modbus_device::utils::get_defs_from_json;

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

#[derive(Debug)]
struct Devices {
    modbus: HashMap<String, Arc<Mutex<ModbusDeviceAsync>>>,
}

// Connect all devices passed as arguments to their targets, panics on error (this should then only be used in the initialisation)
// The connection for all devices is realized in parallel
async fn connect_modbus(
    modbus_devices: Arc<RefCell<HashMap<String, Arc<Mutex<ModbusDeviceAsync>>>>>,
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
    modbus_devices: Arc<RefCell<HashMap<String, Arc<Mutex<ModbusDeviceAsync>>>>>,
) -> HashMap<String, HashMap<String, RegisterValue>> {
    // Create a task for each device
    let mut set = JoinSet::new();
    for (name, device) in modbus_devices.borrow().iter() {
        let d = device.clone();
        let name = name.clone();
        set.spawn(async move {
            info!("Fetching modbus input registers from {name}");
            let data: Result<HashMap<String, RegisterValue>, _> =
                d.lock().await.dump_input_registers().await;
            match data {
                Ok(val) => Some(HashMap::from([(name, val)])),
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
                        res.extend(val.unwrap())
                    }
                }
                Err(err) => error!("There was an error joining the tasks responsible for fetching modbus data ({err})"),
            }
        }
    }
    .await;
    res
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Args::parse();
    let config = config::Config::builder()
        .add_source(config::File::with_name(&args.config_file))
        .build()
        .unwrap();

    let app: AppConfig = config.try_deserialize().unwrap();

    let mut devices = Devices {
        modbus: HashMap::new(),
    };

    for device in &app.devices.modbus {
        let addr = match device.1.remote.parse() {
            Ok(addr) => addr,
            Err(err) => panic!(
                "Invalid remote address entered {0} ({err})",
                device.1.remote
            ),
        };
        let input_registers_json = match File::open(device.1.input_registers.clone()) {
            Ok(file) => file,
            Err(err) => panic!(
                "Could not open the file containing the input registers definition : {0} ({err:?})",
                device.1.input_registers
            ),
        };
        let holding_registers_json = match File::open(device.1.holding_registers.clone()) {
            Ok(file) => file,
            Err(err) => panic!(
                "Could not open the file containing the holding registers definition : {0} ({err:?})",
                device.1.holding_registers
            ),
        };
        let d = ModbusDeviceAsync::new(
            addr,
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
            .insert(device.0.clone(), Arc::new(Mutex::new(d)));
    }

    let modbus_devices = Arc::new(RefCell::new(devices.modbus));
    connect_modbus(modbus_devices.clone()).await;

    let mut interval = tokio::time::interval(Duration::from_secs(app.period));

    loop {
        interval.tick().await;
        let modbus_data = fetch_modbus(modbus_devices.clone()).await;

        debug!("{modbus_data:?}");
    }
}
