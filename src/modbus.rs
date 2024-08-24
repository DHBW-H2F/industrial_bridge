use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use log::{error, info};
use modbus_device::{
    errors::ModbusError, modbus_connexion_async::ModbusConnexionAsync,
    modbus_device_async::ModbusDeviceAsync,
};
use tokio::{sync::Mutex, task::JoinSet};

use crate::types_conversion::{convert_hashmap, RegisterValue};
