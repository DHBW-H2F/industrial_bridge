use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use industrial_device::errors::IndustrialDeviceError;
use industrial_device::IndustrialDevice;
use log::{error, info};
use tokio::{sync::Mutex, task::JoinSet};

use crate::types_conversion::{convert_hashmap, RegisterValue};

// Connect all devices passed as arguments to their targets, panics on error (this should then only be used in the initialisation)
// The connection for all devices is realized in parallel
pub async fn connect_devices<T: IndustrialDevice + Send + 'static>(
    devices: Rc<RefCell<HashMap<String, Arc<Mutex<T>>>>>,
) {
    // Create a task for each target
    let mut set = JoinSet::new();
    for (name, device) in devices.borrow().iter() {
        let d = device.clone();
        let name = name.clone();
        set.spawn(async move {
            let dc = d.clone();
            let mut dm = dc.lock().await;
            let res = dm.connect().await;
            (name, res)
        });
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
async fn manage_errors(
    err: IndustrialDeviceError,
    device: Arc<Mutex<impl IndustrialDevice>>,
) -> Result<(), IndustrialDeviceError> {
    match err {
        IndustrialDeviceError::DeviceNotAccessibleError { err }
        | IndustrialDeviceError::DeviceNotConnectedError { err } => {
            error!("Device not accessible while reading register reconnecting to device ({err})");
            let connection_res = device.lock().await.connect().await;
            return match connection_res {
                Ok(_res) => {
                    info!("Reconnexion successful !");
                    Ok(())
                }
                Err(err) => {
                    error!("Reconnexion failed ({err:?})");
                    Err(err.into())
                }
            };
        }
        IndustrialDeviceError::RequestError { err: _ }
        | IndustrialDeviceError::ConversionError { err: _ } => {
            error!("Error reading registers, skipping this run ({err:?})");
            return Err(err);
        }
    };
}

// For all the devices passed, dump all registers and returns it as a HashMap<device_name, HashMap<register_name, register_value>>
// Calls manage_error on error to try to reconnect
// The data fetch if realized in parallel for each target
pub async fn fetch_device<T: IndustrialDevice + Send + 'static>(
    devices: Rc<RefCell<HashMap<String, Arc<Mutex<T>>>>>,
) -> HashMap<String, HashMap<String, RegisterValue>> {
    // Create a task for each device
    let mut set = JoinSet::new();
    for (name, device) in devices.borrow().iter() {
        let d = device.clone();
        let name = name.clone();
        set.spawn(async move {
            info!("Fetching modbus input registers from {name}");
            let data_input: Result<HashMap<String, industrial_device::types::Value>, _> =
                d.lock().await.dump_registers().await;

            let res: HashMap<String, RegisterValue> = match data_input {
                Ok(val) => HashMap::from(convert_hashmap(val)),
                Err(err) => {
                    let _ = manage_errors(err, d.clone()).await;
                    return HashMap::new();
                }
            };

            HashMap::from([(name, res)])
        });
    }

    // join the tasks and merge the results
    let mut res: HashMap<String, HashMap<String, RegisterValue>> = HashMap::new();
    async {
        while let Some(result) = set.join_next().await {
            match result {
                Ok(val) => {
                        res.extend(val);
                }
                Err(err) => error!("There was an error joining the tasks responsible for fetching device data ({err})"),
            }
        }
    }
    .await;
    res
}
