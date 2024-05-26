use poise::serenity_prelude as serenity;
use serenity::{CreateAttachment, CreateEmbed, Mentionable};

use crate::database::controllers::badactor_model_controller::BadActor;
use crate::database::controllers::serverconfig_model_controller::ServerConfigComplete;
use crate::format;
use crate::util::logger::Logger;
use crate::Context as AppContext;

use super::broadcast_handler::{self, get_broadcast_message};
use super::listener::BroadcastListener;

pub struct SendBroadcastMessageOptions<'a> {
    pub ctx: AppContext<'a>,
    pub broadcast_type: broadcast_handler::BroadcastType,
    pub listener: &'a BroadcastListener,
    pub bad_actor: &'a BadActor,
    pub embed: &'a CreateEmbed,
    pub attachment: &'a Option<CreateAttachment>,
}

pub async fn send_broadcast_message(options: SendBroadcastMessageOptions<'_>) {
    let SendBroadcastMessageOptions {
        ctx,
        broadcast_type,
        listener,
        bad_actor,
        embed,
        attachment,
    } = options;

    let content = get_message_with_pings(broadcast_type.message(), &listener.config, bad_actor);
    let message = get_broadcast_message(&content, embed.clone(), attachment.clone());

    if let Err(e) = listener.log_channel.send_message(ctx, message).await {
        let log_msg = format!(
            "Failed to send broadcast embed to #{} in {}",
            listener.log_channel.name,
            format::display(&listener.config.guild)
        );
        Logger::get().error(ctx, e, log_msg).await;
    }
}

fn get_message_with_pings(
    content: &str,
    config: &ServerConfigComplete,
    bad_actor: &BadActor,
) -> String {
    let reporting_guild = bad_actor.origin_guild_id;
    let current_guild = config.server_config.guild_id;

    // skip the ping in the originating guild
    if reporting_guild == current_guild {
        return content.to_string();
    }

    let content = if let Some(ping_role) = config.server_config.ping_role {
        format!("{content}\n{}", ping_role.mention())
    } else {
        content.to_string()
    };

    if config.server_config.ping_users {
        let user_mentions = config
            .users
            .iter()
            .map(|u| u.mention().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        format!("{content}\n{user_mentions}")
    } else {
        content
    }
}
