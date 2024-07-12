use std::{collections::HashMap, fs::File};

use log::{debug, error, info, warn};

use clap::Parser;

mod app_config;
use app_config::AppConfig;

use modbus_device;

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
    modbus: HashMap<String, modbus_device::ModbusDevice>,
}

fn main() {
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
        let d = modbus_device::ModbusDevice {
            ctx: match modbus_device::connect(addr) {
                Ok(ctx) => ctx,
                Err(err) => panic!("Error connecting to device {addr} ({err})"),
            },
            input_registers: match modbus_device::get_defs_from_json(input_registers_json) {
                Ok(registers) => registers,
                Err(err) => panic!("Could not load input registers definition from file ({err})"),
            },
            holding_registers: match modbus_device::get_defs_from_json(holding_registers_json) {
                Ok(registers) => registers,
                Err(err) => panic!("Could not load holding registers definition from file ({err})"),
            },
            addr,
        };
        devices.modbus.insert(device.0.clone(), d);
    }

    debug!("{devices:?}");
}
