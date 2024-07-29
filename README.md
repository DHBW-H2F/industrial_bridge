# Industrial bridge
Bridge between industrial devices (accessible using ModBus, S7, [...](#Protocols)) and a remote database (Prometheus, InfluxDB, [...](#Databases)).

# Usage
```
Usage: industrial_bridge [OPTIONS]

Options:
  -c, --config-file <CONFIG_FILE>
          Where to find the config file
          
          [default: config.yaml]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Configurations
The configuration is defined in a yaml file using the following format : 
```yaml
period: u64 (Update rate, set to 0 for no delay)
devices:
  modbus:
    TCP:
      device:
        remote: String (Address of the device, to be parsed as a SocketAddr)
        input_registers: String (Path to the input registers definition)
        holding_registers: String (Path to the holding registers definition)
    RTU:
      device:
        port: String (Port where the device is connected)
        slave: u32 (slave id of the device)
        speed: u32 (connexion speed (bauds))
        input_registers: String (Path to the input registers definition)
        holding_registers: String (Path to the holding registers definition)
  S7:
    device:
      remote: String (Address of the device, to be parsed as a SocketAddr)
      holding_registers: String (Path to the db registers definition)
remotes:
  influx_db:
    remote:
      remote: String (Url of the remote)
      bucket: String (Bucket in which to store the data)
      token: String (Access token for the remote)
  prometheus:
    remote:
      remote: String (Url of the remote)
```

For an example see [config.yaml](config.yaml)

## Registers definition
The registers definition are loaded from json using the corresponding libraries ([modbus_device](https://github.com/lkzjdnb/modbus_device) and [s7_device](https://github.com/lkzjdnb/S7_devices)).

# Protocols
The currently supported device protocols are :

- Modbus over TCP
- Modbus over RTU
- S7 (for db blocks)

# Databases
The currently supported remote database are : 

- InfluxDB
- Prometheus (via PushGateway)
