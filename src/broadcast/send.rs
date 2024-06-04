use poise::serenity_prelude as serenity;
use serenity::{CacheHttp, CreateAttachment, CreateEmbed, Mentionable};

use crate::database::controllers::badactor_model_controller::BadActor;
use crate::database::controllers::serverconfig_model_controller::{
    ActionLevel, ServerConfigComplete,
};
use crate::format;
use crate::util::logger::Logger;

use super::broadcast_handler::{self, get_broadcast_message};
use super::listener::BroadcastListener;
use super::moderate::get_moderation_action;

pub struct SendBroadcastMessageOptions<'a> {
    pub broadcast_type: broadcast_handler::BroadcastType,
    pub listener: &'a BroadcastListener,
    pub bad_actor: &'a BadActor,
    pub embed: &'a CreateEmbed,
    pub attachment: &'a Option<CreateAttachment>,
}

pub async fn send_broadcast_message(
    cache_http: impl CacheHttp,
    options: SendBroadcastMessageOptions<'_>,
) {
    let SendBroadcastMessageOptions {
        broadcast_type,
        listener,
        bad_actor,
        embed,
        attachment,
    } = options;
    let action_level = get_moderation_action(
        broadcast_type,
        bad_actor.actor_type,
        &listener.config.server_config,
    );

    let content = get_message_with_pings(broadcast_type.message(), &listener.config, bad_actor, action_level);
    let message = get_broadcast_message(
        &content,
        embed.clone(),
        attachment.clone(),
        action_level,
        broadcast_type,
    );

    if let Err(e) = listener
        .log_channel
        .send_message(&cache_http, message)
        .await
    {
        let log_msg = format!(
            "Failed to send broadcast embed to #{} in {}",
            listener.log_channel.name,
            format::display(&listener.config.guild)
        );
        Logger::get().error(&cache_http, e, log_msg).await;
    }
}

fn get_message_with_pings(
    content: &str,
    config: &ServerConfigComplete,
    bad_actor: &BadActor,
    action_level: ActionLevel,
) -> String {
    let reporting_guild = bad_actor.origin_guild_id;
    let current_guild = config.server_config.guild_id;

    // skip the ping in the originating guild
    if reporting_guild == current_guild {
        return content.to_string();
    }

    // skip the ping if automatic moderation is already happening
    if action_level != ActionLevel::Notify {
        return content.to_string()
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
