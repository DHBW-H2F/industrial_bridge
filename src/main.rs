use core::panic;
use devices::{connect_devices, fetch_device};
use industrial_device::IndustrialDevice;
use remotes::remote::Remote;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use log::{debug, error, info};

use clap::Parser;

use tokio::sync::{watch, Mutex};

use url::Url;

use prometheus_push::prometheus_crate::PrometheusMetricsPusher;

use config;

mod app_config;
use app_config::AppConfig;

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
    let devices_box: HashMap<String, Box<dyn IndustrialDevice + Send>> =
        app.devices.try_into().unwrap();

    let devices: Rc<RefCell<HashMap<String, Arc<Mutex<Box<dyn IndustrialDevice + Send>>>>>> =
        Rc::new(RefCell::new(
            devices_box
                .into_iter()
                .map(|(name, val)| (name, Arc::new(Mutex::new(val))))
                .collect(),
        ));

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

    // connect to all devices
    connect_devices(devices.clone()).await;

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

        rec_out = fetch_device(devices.clone(), timeout).await;

        debug!("{rec_out:?}");

        // Send the new data
        match data_received_tx.send(rec_out) {
            Ok(_) => {}
            Err(err) => error!("Could not send data to be pushed : ({err})"),
        };
    }
}
