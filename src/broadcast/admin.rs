use anyhow::Context;
use poise::serenity_prelude as serenity;
use serenity::{CreateAttachment, CreateEmbed};

use crate::broadcast::broadcast_handler::BroadcastType;
use crate::Context as AppContext;

use super::broadcast_handler::get_broadcast_message;

pub struct BroadcastAdminServerOptions<'a> {
    pub ctx: AppContext<'a>,
    pub embed: CreateEmbed,
    pub attachment: Option<CreateAttachment>,
    pub broadcast_type: BroadcastType,
}

pub async fn broadcast_admin_server(
    options: BroadcastAdminServerOptions<'_>,
) -> anyhow::Result<()> {
    let BroadcastAdminServerOptions {
        attachment,
        broadcast_type,
        ctx,
        embed,
    } = options;

    let message = get_broadcast_message(broadcast_type.message(), embed, attachment);

    ctx.data()
        .config
        .admin_server_log_channel
        .send_message(ctx, message)
        .await
        .context("Failed to broadcast to admin server")?;

    Ok(())
}
