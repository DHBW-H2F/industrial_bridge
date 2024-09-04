use core::panic;
use devices::{connect_devices, fetch_device};
use remotes::remote::Remote;
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
use tokio::sync::{watch, Mutex};

use url::Url;

use prometheus_push::prometheus_crate::PrometheusMetricsPusher;

use modbus_device;
use modbus_device::utils::get_defs_from_json;
use modbus_device::ModbusDeviceAsync;

use s7_device::S7Device;

use config;

mod app_config;
use app_config::{AppConfig, ModbusDevice};

mod types_conversion;
use types_conversion::RegisterValue;

mod devices;
mod remotes;
use remotes::send_data_to_remotes;

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
    let remotes: Arc<Mutex<HashMap<String, Arc<Mutex<dyn Remote + Send>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

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
                remotes.lock().await.insert(name.clone(), remote_dev);
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
                remotes.lock().await.insert(name.clone(), pusher);
            }
        }
    }

    // connect to all modbus devices
    let connect_modbus = connect_devices(devices.modbus.clone());
    // connect to all S7 devices
    let connect_s7 = connect_devices(devices.s7.clone());
    join!(connect_modbus, connect_s7);

    // Data fetch is triggered at the interval entered in configuration
    let mut interval = tokio::time::interval(Duration::from_secs(app.period));

    let timeout = match app.timeout {
        Some(timeout) => Duration::from_secs(timeout),
        None => Duration::MAX,
    };

    let (data_received_tx, mut data_received_rx) =
        watch::channel(HashMap::<String, HashMap<String, RegisterValue>>::new());
    // Start the task that send data to remotes
    {
        tokio::task::spawn(async move {
            match data_received_rx.changed().await {
                Ok(_) => {}
                Err(err) => error!("There was an error waiting for new data : ({err})"),
            };
            send_data_to_remotes(remotes, data_received_rx).await;
        });
    }

    loop {
        // Wait for the configured time
        interval.tick().await;

        // Fetch all data
        let mut rec_out: HashMap<String, HashMap<String, RegisterValue>> = HashMap::new();
        rec_out.clear();

        let fetch_modbus = fetch_device(devices.modbus.clone(), timeout);
        let fetch_s7 = fetch_device(devices.s7.clone(), timeout);

        let (data_modbus, data_s7) = join!(fetch_modbus, fetch_s7);
        rec_out.extend(data_modbus);
        rec_out.extend(data_s7);

        debug!("{rec_out:?}");

        // Send the new data
        match data_received_tx.send(rec_out) {
            Ok(_) => {}
            Err(err) => error!("Could not send data to be pushed : ({err})"),
        };
    }
}
