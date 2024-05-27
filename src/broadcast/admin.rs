use anyhow::Context;
use poise::serenity_prelude as serenity;
use serenity::{CacheHttp, CreateAttachment, CreateEmbed};

use crate::broadcast::broadcast_handler::BroadcastType;
use crate::util::config::Config;

use super::broadcast_handler::get_broadcast_message;

pub struct BroadcastAdminServerOptions<'a> {
    pub config: &'a Config,
    pub embed: CreateEmbed,
    pub attachment: Option<CreateAttachment>,
    pub broadcast_type: BroadcastType,
}

pub async fn broadcast_admin_server(
    cache_http: impl CacheHttp,
    options: BroadcastAdminServerOptions<'_>,
) -> anyhow::Result<()> {
    let BroadcastAdminServerOptions {
        config,
        embed,
        attachment,
        broadcast_type,
    } = options;

    let message = get_broadcast_message(broadcast_type.message(), embed, attachment);

    config
        .admin_server_log_channel
        .send_message(&cache_http, message)
        .await
        .context("Failed to broadcast to admin server")?;

    Ok(())
}
