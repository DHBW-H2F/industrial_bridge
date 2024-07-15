use std::cell::RefCell;
use std::sync::Arc;
use std::{collections::HashMap, fs::File};

use log::{debug, info};

use clap::Parser;

use tokio::sync::Mutex;
use tokio::task::JoinSet;

use std::ops::DerefMut;

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

async fn connect_modbus(
    modbus_devices: Arc<RefCell<HashMap<String, Arc<Mutex<ModbusDeviceAsync>>>>>,
) {
    let mut set = JoinSet::new();
    for (name, device) in modbus_devices.borrow().iter() {
        let d = device.clone();
        let name = name.clone();
        set.spawn(async move {
            d.lock().await.connect().await.unwrap();
            info!("Connected to {name}")
        });
    }

    async { while set.join_next().await.is_some() {} }.await;
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

    let mut modbus_devices_mut = modbus_devices.borrow_mut();
    let mut electrolyzer_ref = modbus_devices_mut
        .get_mut("electrolyzer")
        .unwrap()
        .lock()
        .await;
    let electrolyzer: &mut ModbusDeviceAsync = electrolyzer_ref.deref_mut();
    let reg_dump = electrolyzer.dump_input_registers().await.unwrap();
    debug!("{reg_dump:?}");
}
