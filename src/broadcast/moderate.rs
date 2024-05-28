use chrono::{Days, Utc};
use poise::serenity_prelude as serenity;
use serenity::{
    CacheHttp, CreateMessage, GuildChannel, GuildId, Member, Mentionable, PartialGuild, RoleId,
    User,
};

use crate::database::controllers::badactor_model_controller::{BadActor, BadActorType};
use crate::database::controllers::serverconfig_model_controller::{ActionLevel, ServerConfig};
use crate::util::format;
use crate::util::logger::Logger;

use super::broadcast_handler::BroadcastType;
use super::listener::BroadcastListener;

pub struct ModerateOptions<'a> {
    pub broadcast_type: BroadcastType,
    pub listener: &'a BroadcastListener,
    pub bad_actor: &'a BadActor,
    pub target_user: &'a User,
}

pub async fn moderate(cache_http: impl CacheHttp, options: ModerateOptions<'_>) {
    let ModerateOptions {
        broadcast_type,
        listener,
        bad_actor,
        target_user,
    } = options;

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
        .member(&cache_http, bad_actor.user_id)
        .await
        .ok();

    // the only moderation action we can take on people who are not members it to ban them
    if member.is_none() && action_level == ActionLevel::Ban {
        let _ban_result = ban(
            &cache_http,
            &listener.config.guild,
            target_user,
            &listener.log_channel,
            bad_actor.ban_reason(),
        )
        .await;

        return;
    }

    // inform the guild that the user is not a member
    let Some(mut member) = member else {
        let user_msg = CreateMessage::new().content(format!(
            "User {} is not a member of your server. Skipping moderation.",
            format::fdisplay(target_user)
        ));

        if let Err(e) = listener
            .log_channel
            .send_message(&cache_http, user_msg)
            .await
        {
            let log_msg = format!(
                "Failed to send user not member message to #{} in {}",
                listener.log_channel.name,
                format::display(&listener.config.guild)
            );
            Logger::get().error(&cache_http, e, log_msg).await;
        }

        return;
    };

    let non_ignored_roles = get_non_ignored_roles(
        &member.roles,
        &listener.config.server_config.ignored_roles,
        listener.config.guild.id,
    );

    if !non_ignored_roles.is_empty() {
        inform_about_non_ignored(
            &cache_http,
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
                &cache_http,
                &listener.config.guild,
                &mut member,
                &listener.log_channel,
            )
            .await
        }
        ActionLevel::Kick => {
            kick(
                &cache_http,
                &listener.config.guild,
                &member,
                &listener.log_channel,
            )
            .await
        }
        ActionLevel::SoftBan => {
            soft_ban(
                &cache_http,
                &listener.config.guild,
                target_user,
                &listener.log_channel,
            )
            .await
        }
        ActionLevel::Ban => {
            ban(
                &cache_http,
                &listener.config.guild,
                target_user,
                &listener.log_channel,
                bad_actor.ban_reason(),
            )
            .await
        }
    };

    log_moderation_result(
        &cache_http,
        moderation_result,
        target_user,
        &listener.config.guild,
    )
    .await;
}

async fn log_moderation_result(
    cache_http: impl CacheHttp,
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

        Logger::get().error(cache_http, e, log_msg).await;
    }
}

async fn inform_about_non_ignored(
    cache_http: impl CacheHttp,
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
        .send_message(&cache_http, CreateMessage::new().content(content))
        .await
    {
        let log_msg = format!("Failed to inform {} that the member {} cannot be moderated since they have roles that are not ignored.", format::display(guild), format::display(target_user));
        Logger::get().error(cache_http, e, log_msg).await;
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
    if !broadcast_type.is_new_report() {
        return ActionLevel::Notify;
    }

    match actor_type {
        BadActorType::Spam => server_config.spam_action_level,
        BadActorType::Impersonation => server_config.impersonation_action_level,
        BadActorType::Bigotry => server_config.bigotry_action_level,
        BadActorType::Honeypot => server_config.honeypot_action_level,
    }
}

async fn ban(
    cache_http: impl CacheHttp,
    guild: &PartialGuild,
    target_user: &User,
    log_channel: &GuildChannel,
    reason: impl AsRef<str>,
) -> anyhow::Result<()> {
    guild
        .ban_with_reason(cache_http.http(), target_user, 7, reason)
        .await?;

    tracing::info!(
        "Banned {} from {}.",
        format::display(target_user),
        format::display(guild)
    );

    let user_msg = CreateMessage::new().content(format!(
        "User {} was banned from your server!",
        format::fdisplay(target_user)
    ));

    log_channel.send_message(cache_http, user_msg).await?;

    Ok(())
}

async fn soft_ban(
    cache_http: impl CacheHttp,
    guild: &PartialGuild,
    target_user: &User,
    log_channel: &GuildChannel,
) -> anyhow::Result<()> {
    guild.ban(cache_http.http(), target_user, 7).await?;
    guild.unban(cache_http.http(), target_user).await?;

    tracing::info!(
        "Softbanned {} from {}.",
        format::display(target_user),
        format::display(guild)
    );

    let user_msg = CreateMessage::new().content(format!(
        "User {} was softbanned from your server!",
        format::fdisplay(target_user)
    ));

    log_channel.send_message(cache_http, user_msg).await?;

    Ok(())
}

async fn timeout(
    cache_http: impl CacheHttp,
    guild: &PartialGuild,
    member: &mut Member,
    log_channel: &GuildChannel,
) -> anyhow::Result<()> {
    let in_seven_days = Utc::now() + Days::new(7);
    member
        .disable_communication_until_datetime(&cache_http, in_seven_days.into())
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

    log_channel.send_message(cache_http, user_msg).await?;

    Ok(())
}

async fn kick(
    cache_http: impl CacheHttp,
    guild: &PartialGuild,
    member: &Member,
    log_channel: &GuildChannel,
) -> anyhow::Result<()> {
    member.kick(&cache_http).await?;

    tracing::info!(
        "Kicked {} from {}.",
        format::display(&member.user),
        format::display(guild)
    );

    let user_msg = CreateMessage::new().content(format!(
        "User {} was kicked from your server!",
        format::fdisplay(&member.user)
    ));

    log_channel.send_message(cache_http, user_msg).await?;

    Ok(())
}
