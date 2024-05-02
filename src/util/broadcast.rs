use anyhow::Context;
use chrono::{Days, Utc};
use dashmap::DashMap;
use futures::{stream::FuturesUnordered, StreamExt};
use poise::serenity_prelude as serenity;
use serenity::{
    ChannelId, CreateAttachment, CreateEmbed, CreateMessage, GuildChannel, GuildId, Member,
    Mentionable, PartialGuild, RoleId, User,
};

use crate::database::badactor_model_controller::{BadActor, BadActorType};
use crate::database::serverconfig_model_controller::{
    ActionLevel, ServerConfig, ServerConfigComplete, ServerConfigModelController,
};
use crate::{Context as AppContext, Logger};

use super::format;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BroadcastType {
    Report,
    Deactivate,
    Reactivate,
    AddScreenshot,
    ReplaceScreenshot,
    UpdateExplanation,
}

impl BroadcastType {
    fn message(&self) -> &'static str {
        match self {
            Self::Report => "A bad actor has been reported.",
            Self::Deactivate => "A bad actor has been deactivated.",
            Self::Reactivate => "A bad actor has been reactivated.",
            Self::AddScreenshot => "A screenshot proof has been added to a bad actor entry.",
            Self::UpdateExplanation => "The explanation for a bad actor has been updated.",
            Self::ReplaceScreenshot => "A screenshot has been replaced for a bad actor.",
        }
    }
}

struct BroadcastListener {
    config: ServerConfigComplete,
    log_channel: GuildChannel,
}

type Listeners = DashMap<GuildId, BroadcastListener>;

pub async fn broadcast(
    ctx: AppContext<'_>,
    target_user: &User,
    bad_actor: &BadActor,
    broadcast_type: BroadcastType,
    interaction_guild: &PartialGuild,
) -> anyhow::Result<()> {
    let listeners = get_valid_configs(ctx).await?;

    let (embed, attachment) = bad_actor
        .to_broadcast_embed(ctx, interaction_guild, target_user)
        .await;

    if let Err(e) =
        broadcast_admin_server(ctx, embed.clone(), attachment.clone(), broadcast_type).await
    {
        let log_msg = "Failed to broadcast to admin server log channel";
        Logger::get().error(ctx, e, log_msg).await;
    }

    if let Err(e) = notify_user(ctx, target_user).await {
        let log_msg = format!(
            "Failed to inform {} about the moderation actions in DM",
            format::display(target_user)
        );
        Logger::get().warn(ctx, log_msg).await;
    }

    broadcast_listeners(
        ctx,
        broadcast_type,
        &listeners,
        bad_actor,
        target_user,
        embed,
        attachment,
    )
    .await;

    Ok(())
}

async fn get_valid_configs(ctx: AppContext<'_>) -> anyhow::Result<Vec<BroadcastListener>> {
    let mut config_futures = ServerConfigModelController::get_all(&ctx.data().db_pool)
        .await?
        .into_iter()
        .map(|server_config| async { get_valid_logchannel(ctx, server_config).await })
        .collect::<FuturesUnordered<_>>();

    let mut valid_configs = Vec::new();

    while let Some((guild_id, config_result, log_channel)) = config_futures.next().await {
        match config_result {
            Ok(config) => {
                if let Some(c) = log_channel {
                    valid_configs.push(BroadcastListener {
                        config,
                        log_channel: c,
                    });
                }
            }
            Err(e) => {
                let log_future = async {
                    let log_msg = format!("Failed to upgrade config for {}. Skipping their server for broadcasting: {e}", guild_id);
                    Logger::get().warn(ctx, log_msg).await;
                };
                log_future.await;
            }
        }
    }

    Ok(valid_configs)
}

async fn get_valid_logchannel(
    ctx: AppContext<'_>,
    server_config: ServerConfig,
) -> (
    GuildId,
    anyhow::Result<ServerConfigComplete>,
    Option<GuildChannel>,
) {
    let server_id = server_config.server_id;

    let Some(log_channel) = server_config.log_channel else {
        let err = Err(anyhow::anyhow!(
            "There is no log channel defined for {}",
            server_config.server_id
        ));

        return (server_id, err, None);
    };

    let Ok(log_channel) = log_channel.to_channel(ctx).await else {
        let err = Err(anyhow::anyhow!(
            "Cannot get log channel for {}",
            server_config.server_id
        ));

        return (server_id, err, None);
    };

    let Some(log_channel) = log_channel.guild() else {
        let err = Err(anyhow::anyhow!(
            "Log channel for {} is not a guild channel",
            server_config.server_id
        ));

        return (server_id, err, None);
    };

    if !log_channel.is_text_based() {
        let err = Err(anyhow::anyhow!(
            "Log channel for {} is not a text channel",
            server_config.server_id
        ));

        return (server_id, err, None);
    }

    let complete = ServerConfigComplete::try_from_server_config(server_config, ctx).await;

    (server_id, complete, Some(log_channel))
}

async fn get_guild_text_channel(
    ctx: AppContext<'_>,
    channel_id: ChannelId,
) -> anyhow::Result<GuildChannel> {
    let Ok(channel) = channel_id.to_channel(ctx).await else {
        anyhow::bail!("Cannot get channel {channel_id} from the API");
    };

    let Some(channel) = channel.guild() else {
        anyhow::bail!("Channel {channel_id} is not a guild channel");
    };

    if !channel.is_text_based() {
        anyhow::bail!("Channel {channel_id} is not a text channel");
    }

    Ok(channel)
}

async fn broadcast_admin_server(
    ctx: AppContext<'_>,
    embed: CreateEmbed,
    attachment: Option<CreateAttachment>,
    broadcast_type: BroadcastType,
) -> anyhow::Result<()> {
    let message = get_broadcast_message(broadcast_type.message(), embed, attachment);
    ctx.data()
        .config
        .admin_server_log_channel
        .send_message(ctx, message)
        .await
        .context("Failed to broadcast to admin server")?;

    Ok(())
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

async fn broadcast_listeners(
    ctx: AppContext<'_>,
    broadcast_type: BroadcastType,
    listeners: &[BroadcastListener],
    bad_actor: &BadActor,
    target_user: &User,
    embed: CreateEmbed,
    attachment: Option<CreateAttachment>,
) {
    let futures = listeners.iter().map(|listener| async {
        tokio::join!(
            send_broadcast_message(
                ctx,
                broadcast_type,
                listener,
                bad_actor,
                &embed,
                &attachment
            ),
            moderate(ctx, broadcast_type, listener, bad_actor, target_user)
        );
    });

    futures::future::join_all(futures).await;
}

async fn send_broadcast_message(
    ctx: AppContext<'_>,
    broadcast_type: BroadcastType,
    listener: &BroadcastListener,
    bad_actor: &BadActor,
    embed: &CreateEmbed,
    attachment: &Option<CreateAttachment>,
) {
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

async fn moderate(
    ctx: AppContext<'_>,
    broadcast_type: BroadcastType,
    listener: &BroadcastListener,
    bad_actor: &BadActor,
    target_user: &User,
) {
    let action_level = get_moderation_action(
        broadcast_type,
        bad_actor.actor_type,
        &listener.config.server_config,
    );

    if let ActionLevel::Notify = action_level {
        return;
    }

    let member = listener
        .config
        .guild
        .member(ctx, bad_actor.user_id)
        .await
        .ok();

    // the only moderation action we can take on people who are not members it to ban them
    if member.is_none() && action_level == ActionLevel::Ban {
        let ban_result = ban(
            ctx,
            &listener.config.guild,
            target_user,
            &listener.log_channel,
            bad_actor.ban_reason(),
        )
        .await;

        return;
    }

    // inform the guild that the user is not a member
    if member.is_none() {
        let user_msg = CreateMessage::new().content(format!(
            "User {} is not a member of your server. Skipping moderation.",
            format::fdisplay(target_user)
        ));

        if let Err(e) = listener.log_channel.send_message(ctx, user_msg).await {
            let log_msg = format!(
                "Failed to send user not member message to #{} in {}",
                listener.log_channel.name,
                format::display(&listener.config.guild)
            );
            Logger::get().error(ctx, e, log_msg).await;
        }

        return;
    }

    let mut member = member.unwrap();

    let non_ignored_roles = get_non_ignored_roles(
        &member.roles,
        &listener.config.server_config.ignored_roles,
        listener.config.guild.id,
    );

    if !non_ignored_roles.is_empty() {
        inform_about_non_ignored(
            ctx,
            &non_ignored_roles,
            &listener.log_channel,
            &listener.config.guild,
            target_user,
        )
        .await;

        return;
    }

    let moderation_result = match action_level {
        ActionLevel::Notify => Ok(()),
        ActionLevel::Timeout => {
            timeout(
                ctx,
                &listener.config.guild,
                &mut member,
                &listener.log_channel,
            )
            .await
        }
        ActionLevel::Kick => {
            kick(ctx, &listener.config.guild, &member, &listener.log_channel).await
        }
        ActionLevel::SoftBan => {
            soft_ban(
                ctx,
                &listener.config.guild,
                target_user,
                &listener.log_channel,
            )
            .await
        }
        ActionLevel::Ban => {
            ban(
                ctx,
                &listener.config.guild,
                target_user,
                &listener.log_channel,
                bad_actor.ban_reason(),
            )
            .await
        }
    };

    log_moderation_result(ctx, moderation_result, target_user, &listener.config.guild).await;
}

async fn log_moderation_result(
    ctx: AppContext<'_>,
    result: anyhow::Result<()>,
    target_user: &User,
    guild: &PartialGuild,
) {
    if let Err(e) = result {
        let log_msg = format!(
            "Error moderating {} in {}",
            format::display(target_user),
            format::display(guild)
        );

        Logger::get().error(ctx, e, log_msg).await;
    }
}

async fn inform_about_non_ignored(
    ctx: AppContext<'_>,
    non_ignored_roles: &[RoleId],
    log_channel: &GuildChannel,
    guild: &PartialGuild,
    target_user: &User,
) {
    let roles = non_ignored_roles
        .iter()
        .map(|r| r.mention().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let content = format!("User {} has roles that are not ignored. Those roles are {roles}. Skipping all moderation action.", format::fdisplay(target_user));

    if let Err(e) = log_channel
        .send_message(ctx, CreateMessage::new().content(content))
        .await
    {
        let log_msg = format!("Failed to inform {} that the member {} cannot be moderated since they have roles that are not ignored.", format::display(guild), format::display(target_user));
        Logger::get().error(ctx, e, log_msg).await;
    }
}

fn get_non_ignored_roles(
    member_roles: &[RoleId],
    ignored_roles: &[RoleId],
    guild_id: GuildId,
) -> Vec<RoleId> {
    let mut non_ignored_roles: Vec<RoleId> = Vec::new();

    for &role in member_roles {
        if role == guild_id.everyone_role() {
            continue;
        }

        if !ignored_roles.contains(&role) {
            non_ignored_roles.push(role)
        }
    }

    non_ignored_roles
}

fn get_moderation_action(
    broadcast_type: BroadcastType,
    actor_type: BadActorType,
    server_config: &ServerConfig,
) -> ActionLevel {
    if broadcast_type != BroadcastType::Report {
        return ActionLevel::Notify;
    }

    match actor_type {
        BadActorType::Spam => server_config.spam_action_level,
        BadActorType::Impersonation => server_config.impersonation_action_level,
        BadActorType::Bigotry => server_config.bigotry_action_level,
    }
}

fn get_message_with_pings(
    content: &str,
    config: &ServerConfigComplete,
    bad_actor: &BadActor,
) -> String {
    let reporting_guild = bad_actor.originally_created_in;
    let current_guild = config.server_config.server_id;

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

fn get_broadcast_message(
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

async fn ban(
    ctx: AppContext<'_>,
    guild: &PartialGuild,
    target_user: &User,
    log_channel: &GuildChannel,
    reason: impl AsRef<str>,
) -> anyhow::Result<()> {
    guild.ban_with_reason(ctx, target_user, 7, reason).await?;

    tracing::info!(
        "Banned {} from {}.",
        format::display(target_user),
        format::display(guild)
    );

    let user_msg = CreateMessage::new().content(format!(
        "User {} was banned from your server!",
        format::fdisplay(target_user)
    ));

    log_channel.send_message(ctx, user_msg).await?;

    Ok(())
}

async fn soft_ban(
    ctx: AppContext<'_>,
    guild: &PartialGuild,
    target_user: &User,
    log_channel: &GuildChannel,
) -> anyhow::Result<()> {
    guild.ban(ctx, target_user, 7).await?;
    guild.unban(ctx, target_user).await?;

    tracing::info!(
        "Softbanned {} from {}.",
        format::display(target_user),
        format::display(guild)
    );

    let user_msg = CreateMessage::new().content(format!(
        "User {} was softbanned from your server!",
        format::fdisplay(target_user)
    ));

    log_channel.send_message(ctx, user_msg).await?;

    Ok(())
}

async fn timeout(
    ctx: AppContext<'_>,
    guild: &PartialGuild,
    member: &mut Member,
    log_channel: &GuildChannel,
) -> anyhow::Result<()> {
    let in_seven_days = Utc::now() + Days::new(7);
    member
        .disable_communication_until_datetime(ctx, in_seven_days.into())
        .await?;

    tracing::info!(
        "Timed out {} in {}.",
        format::display(&member.user),
        format::display(guild)
    );

    let user_msg = CreateMessage::new().content(format!(
        "User {} was timed out for 7 days!",
        format::fdisplay(&member.user)
    ));

    log_channel.send_message(ctx, user_msg).await?;

    Ok(())
}

async fn kick(
    ctx: AppContext<'_>,
    guild: &PartialGuild,
    member: &Member,
    log_channel: &GuildChannel,
) -> anyhow::Result<()> {
    member.kick(ctx).await?;

    tracing::info!(
        "Kicked {} from {}.",
        format::display(&member.user),
        format::display(guild)
    );

    let user_msg = CreateMessage::new().content(format!(
        "User {} was kicked from your server!",
        format::fdisplay(&member.user)
    ));

    log_channel.send_message(ctx, user_msg).await?;

    Ok(())
}
