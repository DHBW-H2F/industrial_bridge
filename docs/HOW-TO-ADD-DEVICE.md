# Definition
Create a file in the src/devices/ directory named your-device.rs (ex: src/devices/modbus.rs)

Add a struct that will define the config for this device, ex :
```rust
#[derive(Deserialize, Debug, Clone)]
pub struct ModbusTCPDevice {
    pub remote: String,
    pub input_registers: String,
    pub holding_registers: String,
}
```

Implement a way to initialise the control object from the config, ex :
```rust
impl TryFrom<ModbusTCPDevice> for ModbusDeviceAsync {
    type Error = DeviceInitError;

    fn try_from(value: ModbusTCPDevice) -> Result<Self, Self::Error> {
        let input_registers_json = File::open(value.input_registers)?;
        let input_registers = get_defs_from_json(input_registers_json)?;

        let holding_registers_json = File::open(value.holding_registers)?;
        let holding_registers = get_defs_from_json(holding_registers_json)?;

        let addr: SocketAddr = value.remote.parse()?;
        let context = TCPContext { addr };

        Ok(ModbusDeviceAsync::new(
            context.into(),
            input_registers,
            holding_registers,
        ))
    }
}
```

# Add your created file to the devices.rs
```rust
pub mod your_file_name_without_trailing_dot_rs;
```

# Add a field to the config definition (app_config.rs)
```diff
#[derive(Deserialize, Debug)]
pub struct Devices {
+    #[device(ModbusDeviceAsync)]
+    pub modbus_tcp: Option<HashMap<String, ModbusTCPDevice>>,
}
```

# Finally add your devices to the configuration (config.yaml)
```yaml
devices:
    modbus_tcp:
        electrolyzer:
            remote: "127.0.0.1:4502"
            input_registers: "input_registers.json"
            holding_registers: "holding_registers.json"
```
