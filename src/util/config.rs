use anyhow::Context;
use serde::Deserialize;
use serenity::all::{ChannelId, GuildId, UserId};

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub bot_token: String,
    pub database_url: String,
    pub superuser: UserId,
    pub admins_server_id: GuildId,
    pub admin_server_log_channel: ChannelId,
    pub admin_server_error_log_channel: ChannelId,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let file = std::fs::File::open("config.json")?;
        let reader = std::io::BufReader::new(file);

        serde_json::from_reader(reader).context("Failed to parse config file")
    }
}
