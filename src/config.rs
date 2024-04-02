use anyhow::Context;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub bot_token: String,
    pub database_url: String,
    pub superuser: String,
    pub admins_server_id: String,
    pub admin_server_log_channel: String,
    pub admin_server_error_log_channel: String,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let file = std::fs::File::open("config.json")?;
        let reader = std::io::BufReader::new(file);

        serde_json::from_reader(reader).context("Failed to parse config file")
    }
}
