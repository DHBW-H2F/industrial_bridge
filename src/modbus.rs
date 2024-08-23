use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use log::{error, info};
use modbus_device::{
    errors::ModbusError, modbus_connexion_async::ModbusConnexionAsync,
    modbus_device_async::ModbusDeviceAsync,
};
use tokio::{sync::Mutex, task::JoinSet};

use crate::types_conversion::{convert_hashmap, RegisterValue};

// Manage errors occuring on a modbus data read, try to reconnect if a BrokenPipe is detected
async fn manage_modbus_error(
    err: ModbusError,
    device: Arc<Mutex<ModbusDeviceAsync>>,
) -> Result<(), ModbusError> {
    match err {
        ModbusError::ModbusError(tokio_modbus::Error::Transport(err)) => match err.kind() {
            std::io::ErrorKind::BrokenPipe => {
                error!("Broken pipe while reading register reconnecting to device ({err})");
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
pub async fn fetch_modbus(
    modbus_devices: Rc<RefCell<HashMap<String, Arc<Mutex<ModbusDeviceAsync>>>>>,
) -> HashMap<String, HashMap<String, RegisterValue>> {
    // Create a task for each device
    let mut set = JoinSet::new();
    for (name, device) in modbus_devices.borrow().iter() {
        let d = device.clone();
        let name = name.clone();
        set.spawn(async move {
            info!("Fetching modbus input registers from {name}");
            let data_input: Result<HashMap<String, modbus_device::types::RegisterValue>, _> =
                d.lock().await.dump_input_registers().await;

            let mut res: HashMap<String, RegisterValue> = match data_input {
                Ok(val) => HashMap::from(convert_hashmap(val)),
                Err(err) => {
                    let _ = manage_modbus_error(err, d.clone()).await;
                    return HashMap::new();
                }
            };

            let data_holding: Result<HashMap<String, modbus_device::types::RegisterValue>, _> =
                d.lock().await.dump_holding_registers().await;
            match data_holding {
                Ok(val) => res.extend(convert_hashmap(val)),
                Err(err) => {
                    let _ = manage_modbus_error(err, d.clone()).await;
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
                Err(err) => error!("There was an error joining the tasks responsible for fetching modbus data ({err})"),
            }
        }
    }
    .await;
    res
}
