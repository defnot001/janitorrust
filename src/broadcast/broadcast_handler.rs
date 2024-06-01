use poise::serenity_prelude as serenity;
use serenity::{
    CacheHttp, CreateActionRow, CreateAttachment, CreateButton, CreateEmbed, CreateMessage,
    GuildId, PartialGuild, User, UserId,
};
use sqlx::PgPool;

use crate::database::controllers::badactor_model_controller::{BadActor, BroadcastEmbedOptions};
use crate::database::controllers::serverconfig_model_controller::ActionLevel;
use crate::util::embeds::EmbedColor;
use crate::util::{config, format, logger};

use super::listener::BroadcastListener;
use super::moderate::ModerateOptions;
use super::send::SendBroadcastMessageOptions;
use super::webhooks::BroadcastWebhookOptions;
use super::{admin, listener, moderate, send, webhooks};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BroadcastType {
    Report,
    Deactivate,
    AddScreenshot,
    ReplaceScreenshot,
    UpdateExplanation,
    Honeypot,
}

impl BroadcastType {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Report => "A bad actor has been reported.",
            Self::Deactivate => "A bad actor has been deactivated.",
            Self::AddScreenshot => "A screenshot proof has been added to a bad actor entry.",
            Self::UpdateExplanation => "The explanation for a bad actor has been updated.",
            Self::ReplaceScreenshot => "A screenshot has been replaced for a bad actor.",
            Self::Honeypot => "A bad actor was caught by the honeypot.",
        }
    }

    pub fn is_new_report(&self) -> bool {
        match self {
            Self::Report | Self::Honeypot => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct BroadcastOptions<'a> {
    pub config: &'a config::Config,
    pub db_pool: &'a PgPool,
    pub reporting_user: &'a User,
    pub reporting_bot_id: UserId,
    pub bad_actor: &'a BadActor,
    pub bad_actor_user: &'a User,
    pub origin_guild: Option<PartialGuild>,
    pub origin_guild_id: GuildId,
    pub broadcast_type: BroadcastType,
}

struct BroadcastToListenersOptions<'a> {
    db_pool: &'a PgPool,
    broadcast_type: BroadcastType,
    listeners: &'a [BroadcastListener],
    bad_actor: &'a BadActor,
    target_user: &'a User,
    embed: CreateEmbed,
    attachment: Option<CreateAttachment>,
}

pub async fn broadcast(cache_http: impl CacheHttp, options: BroadcastOptions<'_>) {
    let BroadcastOptions {
        config,
        db_pool,
        reporting_user,
        reporting_bot_id,
        bad_actor,
        bad_actor_user,
        origin_guild,
        origin_guild_id,
        broadcast_type,
    } = options;

    let listeners = match listener::get_valid_listeners(&cache_http, db_pool).await {
        Ok(listeners) => listeners,
        Err(e) => {
            let log_msg = "Failed to get valid listeners from the database";
            logger::Logger::get().error(&cache_http, e, log_msg).await;
            return;
        }
    };

    let embed_options = BroadcastEmbedOptions {
        origin_guild_id,
        origin_guild,
        report_author: reporting_user,
        bot_id: reporting_bot_id,
    };

    let embed_colour = get_embed_colour(broadcast_type);

    let (embed, attachment) = bad_actor
        .to_broadcast_embed(&cache_http, embed_options, embed_colour)
        .await;

    let admin_options = admin::BroadcastAdminServerOptions {
        config,
        embed: embed.clone(),
        attachment: attachment.clone(),
        broadcast_type,
    };

    if let Err(e) = admin::broadcast_admin_server(&cache_http, admin_options).await {
        let log_msg = "Failed to broadcast to admin server log channel";
        logger::Logger::get().error(&cache_http, e, log_msg).await;
    }

    if broadcast_type.is_new_report() && notify_user(&cache_http, bad_actor_user).await.is_err() {
        let log_msg = format!(
            "Failed to inform {} about the moderation actions in DM",
            format::display(bad_actor_user)
        );
        logger::Logger::get().warn(&cache_http, log_msg).await;
    }

    let listener_options = BroadcastToListenersOptions {
        db_pool,
        broadcast_type,
        listeners: &listeners,
        bad_actor,
        target_user: bad_actor_user,
        embed,
        attachment,
    };

    broadcast_to_listeners(&cache_http, listener_options).await;
}

async fn broadcast_to_listeners(
    cache_http: impl CacheHttp,
    options: BroadcastToListenersOptions<'_>,
) {
    let BroadcastToListenersOptions {
        db_pool,
        broadcast_type,
        listeners,
        bad_actor,
        target_user,
        embed,
        attachment,
    } = options;

    let futures = listeners.iter().map(|listener| async {
        let send_options = SendBroadcastMessageOptions {
            broadcast_type,
            listener,
            bad_actor,
            embed: &embed,
            attachment: &attachment,
        };

        let moderate_options = ModerateOptions {
            broadcast_type,
            listener,
            bad_actor,
            target_user,
        };

        let webhooks_options = BroadcastWebhookOptions {
            db_pool,
            broadcast_type,
            embed: &embed,
            attachment: &attachment,
        };

        tokio::join!(
            send::send_broadcast_message(&cache_http, send_options),
            moderate::moderate(&cache_http, moderate_options),
            webhooks::broadcast_to_webhooks(&cache_http, webhooks_options)
        );
    });

    futures::future::join_all(futures).await;
}

pub fn get_broadcast_message(
    content: &str,
    embed: CreateEmbed,
    attachment: Option<CreateAttachment>,
    action_level: ActionLevel,
    broadcast_type: BroadcastType,
) -> CreateMessage {
    let mut buttons = Vec::new();

    if broadcast_type.is_new_report() && action_level == ActionLevel::Notify {
        buttons.push(CreateButton::new("ban").label("Ban"));
        buttons.push(CreateButton::new("softban").label("Softban"));
        buttons.push(CreateButton::new("kick").label("Kick"));
    } else if broadcast_type == BroadcastType::Deactivate {
        buttons.push(CreateButton::new("unban").label("Unban"));
    }

    let button_len = buttons.len();
    let action_row = CreateActionRow::Buttons(buttons);

    let message = CreateMessage::new().content(content).embed(embed);

    // add the screenshot to the embed
    let message = if let Some(attachment) = attachment {
        message.add_file(attachment)
    } else {
        message
    };

    // add the buttons to the embed and return the message
    if button_len > 0 {
        message.components(vec![action_row])
    } else {
        message
    }
}

pub fn get_broadcast_message_no_buttons(
    content: &str,
    embed: CreateEmbed,
    attachment: Option<CreateAttachment>,
) -> CreateMessage {
    if let Some(attachment) = attachment {
        CreateMessage::new()
            .content(content)
            .embed(embed)
            .add_file(attachment)
    } else {
        CreateMessage::new().content(content).embed(embed)
    }
}

async fn notify_user(cache_http: impl CacheHttp, target_user: &User) -> anyhow::Result<()> {
    let content = "It appears your account has been compromised and used as a spam bot.\n\nAs part of a collaborative effort to more efficiently moderate TMC servers, the actions as listed in the embed have been taken against your account.\nSince not all guilds have automatic moderation, it's possible that you have been banned from more servers than listed.\n\nIf you have now recovered your account, please join this server (https://discord.gg/7tp82FGk3n).\nFollow the instructions there to clear your name and remove the bans on your account.";

    target_user
        .direct_message(cache_http, CreateMessage::new().content(content))
        .await?;

    Ok(())
}

fn get_embed_colour(broadcast_type: BroadcastType) -> EmbedColor {
    match broadcast_type {
        BroadcastType::AddScreenshot => EmbedColor::Yellow,
        BroadcastType::Deactivate => EmbedColor::Green,
        BroadcastType::Honeypot => EmbedColor::DeepPink,
        BroadcastType::Report => EmbedColor::Red,
        BroadcastType::ReplaceScreenshot => EmbedColor::Orange,
        BroadcastType::UpdateExplanation => EmbedColor::Orange,
    }
}
