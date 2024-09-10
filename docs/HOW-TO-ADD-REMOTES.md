# Interface
Define your communication with the remote using a object that implements the [Remote](src/remotes/remote.rs).

# Definition
Define the configuration associated to your remote, ex :
```rust
pub struct InfluxDBRemote {
    pub remote: String,
    pub bucket: String,
    pub token: String,
}
```

Implement the initilisation from config, ex : 
```rust
impl TryFrom<InfluxDBRemote> for Client {
    type Error = Infallible;

    fn try_from(value: InfluxDBRemote) -> Result<Self, Self::Error> {
        let client = Client::new(value.remote, value.bucket).with_token(value.token);
        Ok(client)
    }
}
```

# Add module
Add the module to remotes.rs
```rust
pub mod your_file_name_without_trailing_dot_rs;
```

# Add the definition to config
```diff
#[derive(Deserialize, Debug, IntoHashMap)]
pub struct Remotes {
+    #[device(Client)]
+    pub influx_db: Option<HashMap<String, InfluxDBRemote>>,
}
```
