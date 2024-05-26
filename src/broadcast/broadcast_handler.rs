use poise::serenity_prelude as serenity;
use serenity::{CreateAttachment, CreateEmbed, CreateMessage, PartialGuild, User};

use crate::database::controllers::badactor_model_controller::BadActor;
use crate::util::format;
use crate::util::logger::Logger;
use crate::Context as AppContext;

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
}

impl BroadcastType {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Report => "A bad actor has been reported.",
            Self::Deactivate => "A bad actor has been deactivated.",
            Self::AddScreenshot => "A screenshot proof has been added to a bad actor entry.",
            Self::UpdateExplanation => "The explanation for a bad actor has been updated.",
            Self::ReplaceScreenshot => "A screenshot has been replaced for a bad actor.",
        }
    }
}

#[derive(Debug)]
pub struct BroadcastOptions<'a> {
    pub ctx: AppContext<'a>,
    pub target_user: &'a User,
    pub bad_actor: &'a BadActor,
    pub interaction_guild: &'a PartialGuild,
    pub broadcast_type: BroadcastType,
}

struct BroadcastToListenersOptions<'a> {
    ctx: AppContext<'a>,
    broadcast_type: BroadcastType,
    listeners: &'a [BroadcastListener],
    bad_actor: &'a BadActor,
    target_user: &'a User,
    embed: CreateEmbed,
    attachment: Option<CreateAttachment>,
}

pub async fn broadcast(options: BroadcastOptions<'_>) {
    let BroadcastOptions {
        bad_actor,
        broadcast_type,
        ctx,
        interaction_guild,
        target_user,
    } = options;

    let listeners = match listener::get_valid_listeners(ctx).await {
        Ok(listeners) => listeners,
        Err(e) => {
            let log_msg = "Failed to get valid listeners from the database";
            Logger::get().error(ctx, e, log_msg).await;
            return;
        }
    };

    let (embed, attachment) = bad_actor
        .to_broadcast_embed(
            ctx,
            interaction_guild.id,
            Some(interaction_guild),
            target_user,
        )
        .await;

    let admin_options = admin::BroadcastAdminServerOptions {
        ctx,
        embed: embed.clone(),
        attachment: attachment.clone(),
        broadcast_type,
    };

    if let Err(e) = admin::broadcast_admin_server(admin_options).await {
        let log_msg = "Failed to broadcast to admin server log channel";
        Logger::get().error(ctx, e, log_msg).await;
    }

    if broadcast_type == BroadcastType::Report && notify_user(ctx, target_user).await.is_err() {
        let log_msg = format!(
            "Failed to inform {} about the moderation actions in DM",
            format::display(target_user)
        );
        Logger::get().warn(ctx, log_msg).await;
    }

    let listener_options = BroadcastToListenersOptions {
        ctx,
        broadcast_type,
        listeners: &listeners,
        bad_actor,
        target_user,
        embed,
        attachment,
    };

    broadcast_to_listeners(listener_options).await;
}

async fn broadcast_to_listeners(options: BroadcastToListenersOptions<'_>) {
    let BroadcastToListenersOptions {
        ctx,
        broadcast_type,
        listeners,
        bad_actor,
        target_user,
        embed,
        attachment,
    } = options;

    let futures = listeners.iter().map(|listener| async {
        let send_options = SendBroadcastMessageOptions {
            ctx,
            broadcast_type,
            listener,
            bad_actor,
            embed: &embed,
            attachment: &attachment,
        };

        let moderate_options = ModerateOptions {
            ctx,
            broadcast_type,
            listener,
            bad_actor,
            target_user,
        };

        let webhooks_options = BroadcastWebhookOptions {
            ctx,
            broadcast_type,
            embed: &embed,
            attachment: &attachment,
        };

        tokio::join!(
            send::send_broadcast_message(send_options),
            moderate::moderate(moderate_options),
            webhooks::broadcast_to_webhooks(webhooks_options)
        );
    });

    futures::future::join_all(futures).await;
}

pub fn get_broadcast_message(
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

async fn notify_user(ctx: AppContext<'_>, target_user: &User) -> anyhow::Result<()> {
    let content = r#"
        It appears your account has been compromised and used as a spam bot.
        As part of a collaborative effort to more efficiently moderate TMC servers, the actions as listed in the embed have been taken against your account.
        Since not all guilds have automatic moderation, it's possible that you have been banned from more servers than listed.
        If you have now recovered your account, please join this server (https://discord.gg/7tp82FGk3n).
        Follow the instructions there to clear your name and remove the bans on your account.
        "#;

    target_user
        .direct_message(ctx, CreateMessage::new().content(content))
        .await?;

    Ok(())
}
