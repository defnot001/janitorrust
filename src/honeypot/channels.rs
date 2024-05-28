use dashmap::DashSet;
use serenity::all::ChannelId;
use sqlx::PgPool;
use std::{str::FromStr, sync::Arc};

pub type HoneypotChannels = Arc<DashSet<ChannelId>>;

pub async fn populate_honeypot_channels(channels: &HoneypotChannels, db_pool: &PgPool) {
    channels.clear();

    let snowflakes =
        sqlx::query_scalar::<_, Option<String>>("SELECT honeypot_channel_id FROM server_configs;")
            .fetch_all(db_pool)
            .await
            .expect("Failed to get the honeypot channel ids from the database");

    if snowflakes.is_empty() {
        return;
    }

    for snowflake in snowflakes {
        let Some(snowflake) = snowflake else {
            continue;
        };

        let channel_id = ChannelId::from_str(&snowflake)
            .expect("Failed to parse honeypot channel id snowflake from database");

        channels.insert(channel_id);
    }
}
