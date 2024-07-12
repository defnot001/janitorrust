use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use poise::{serenity_prelude as serenity, FrameworkContext};
use serenity::{
    Cache, CacheHttp, ChannelId, Context, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
    CreateMessage, GuildChannel, GuildId, Message, PartialGuild, Timestamp, User, UserId,
};
use sqlx::PgPool;
use tokio::sync::{Mutex, MutexGuard};

use crate::broadcast::broadcast_handler::{broadcast, BroadcastOptions, BroadcastType};
use crate::database::controllers::badactor_model_controller::{
    BadActor, BadActorModelController, BadActorType, CreateBadActorOptions,
};
use crate::database::controllers::serverconfig_model_controller::ServerConfigModelController;
use crate::util::config::Config;
use crate::util::embeds::EmbedColor;
use crate::util::format::{self, escape_markdown};
use crate::util::logger::Logger;
use crate::Data;

pub type Queue = Arc<Mutex<Vec<HoneypotMessage>>>;

#[derive(Debug)]
pub struct HoneypotMessage {
    pub guild_id: GuildId,
    pub user_id: UserId,
    pub channel_id: ChannelId,
    pub content: String,
    pub timestamp: Instant,
    pub is_in_honeypot: bool,
}

#[derive(Debug)]
struct MaybeReportBadActorOptions<'a> {
    should_report: bool,
    db_pool: &'a PgPool,
    config: &'a Config,
    target_user: &'a User,
    origin_guild_id: GuildId,
    bot_id: UserId,
}

pub async fn handle_message(
    ctx: &Context,
    framework: FrameworkContext<'_, Data, anyhow::Error>,
    msg: &Message,
) {
    let Some(guild_id) = msg.guild_id else {
        return;
    };

    if msg.author.id == framework.bot_id {
        return;
    }

    let is_in_honeypot = framework
        .user_data
        .honeypot_channels
        .contains(&msg.channel_id);

    if is_in_honeypot {
        delete_msg_from_honeypot(&ctx, &ctx, &framework.user_data.db_pool, msg, guild_id).await;
    }

    let mut queue = framework.user_data.queue.lock().await;
    let now = Instant::now();

    let removed_honeypot_messages = remove_old_messages(&mut queue, now);

    let new_honeypot_msg = HoneypotMessage {
        guild_id,
        user_id: msg.author.id,
        content: msg.content.clone(),
        is_in_honeypot,
        channel_id: msg.channel_id,
        timestamp: now,
    };

    let should_report = should_report(&queue, &new_honeypot_msg);
    queue.push(new_honeypot_msg);

    // drop the MutexGuard which unlocks the mutex again
    drop(queue);

    let report_options = MaybeReportBadActorOptions {
        should_report,
        db_pool: &framework.user_data.db_pool,
        config: &framework.user_data.config,
        target_user: &msg.author,
        origin_guild_id: guild_id,
        bot_id: framework.bot_id,
    };

    let report_future = maybe_report_bad_actor(&ctx, report_options);
    let timeout_future = timeout_honeypot_trolls(
        &ctx,
        &framework.user_data.db_pool,
        removed_honeypot_messages,
    );

    tokio::join!(report_future, timeout_future);
}

// Removes all messages that er older than 1 minute from the queue and returns all messages there were sent in the honeypot channel.
// We need this to find out who to timeout.
fn remove_old_messages(
    queue: &mut MutexGuard<'_, Vec<HoneypotMessage>>,
    now: Instant,
) -> Vec<HoneypotMessage> {
    let first_new_msg = queue
        .iter()
        .enumerate()
        .find(|(_, msg)| now - msg.timestamp < Duration::from_secs(60))
        .map(|(i, _)| i)
        .unwrap_or(queue.len());

    queue
        .drain(..first_new_msg)
        .filter(|msg| msg.is_in_honeypot)
        .collect::<Vec<_>>()
}

fn should_report(queue: &MutexGuard<'_, Vec<HoneypotMessage>>, new_msg: &HoneypotMessage) -> bool {
    let mut is_any_in_honeypot = new_msg.is_in_honeypot;

    let mut seen_channel_ids = Vec::with_capacity(3);

    seen_channel_ids.push(new_msg.channel_id);

    for queue_msg in queue.iter() {
        if queue_msg.user_id == new_msg.user_id && queue_msg.content == new_msg.content {
            if !seen_channel_ids.contains(&queue_msg.channel_id) {
                is_any_in_honeypot |= queue_msg.is_in_honeypot;
                seen_channel_ids.push(queue_msg.channel_id);
            }
        }
    }

    seen_channel_ids.len() >= 3 && is_any_in_honeypot
}

async fn maybe_report_bad_actor(
    cache_http: impl CacheHttp,
    options: MaybeReportBadActorOptions<'_>,
) {
    let MaybeReportBadActorOptions {
        should_report,
        db_pool,
        config,
        target_user,
        origin_guild_id,
        bot_id,
    } = options;

    if should_report {
        if has_active_case(&cache_http, db_pool, target_user).await {
            return;
        }

        let bad_actor_options = CreateBadActorOptions {
            user_id: target_user.id,
            actor_type: BadActorType::Honeypot,
            screenshot_proof: None,
            explanation: Some("reached into the honeypot".to_string()),
            origin_guild_id,
            updated_by_user_id: bot_id,
        };

        let bad_actor_future = save_bad_actor(&cache_http, db_pool, target_user, bad_actor_options);
        let bot_user_future = get_bot_user(&cache_http, bot_id);
        let origin_guild_future = get_origin_guild(&cache_http, origin_guild_id);

        let (bad_actor, bot_user, origin_guild) =
            tokio::join!(bad_actor_future, bot_user_future, origin_guild_future);

        let Ok(bot_user) = bot_user else {
            return;
        };

        let Ok(bad_actor) = bad_actor else {
            return;
        };

        let broadcast_options = BroadcastOptions {
            config,
            db_pool,
            reporting_user: &bot_user,
            reporting_bot_id: bot_user.id,
            bad_actor: &bad_actor,
            bad_actor_user: target_user,
            origin_guild,
            origin_guild_id,
            broadcast_type: BroadcastType::Honeypot,
        };

        broadcast(&cache_http, broadcast_options).await;
    }
}

async fn timeout_honeypot_trolls(
    cache_http: impl CacheHttp,
    pg_pool: &PgPool,
    messages: Vec<HoneypotMessage>,
) {
    if messages.is_empty() {
        return;
    }

    let configs_iter = messages
        .iter()
        .map(|m| ServerConfigModelController::get_by_guild_id(pg_pool, m.guild_id));

    let configs = futures::future::join_all(configs_iter)
        .await
        .into_iter()
        .filter_map(|config| match config {
            Ok(Some(config)) => Some(config),
            Ok(None) => {
                tracing::warn!("Server config for honypot timeout does not exist");
                None
            }
            Err(e) => {
                tracing::error!("Error getting config for honeypot timeout: {e}");
                None
            }
        })
        .collect::<Vec<_>>();

    for message in messages {
        let Some(server_config) = configs.iter().find(|&c| c.guild_id == message.guild_id) else {
            tracing::warn!("Cannot find server config for guild {}", message.guild_id);
            continue;
        };

        if server_config.honeypot_timeout.is_zero() {
            continue;
        }

        let Some(log_channel_id) = server_config.log_channel_id else {
            continue;
        };

        let Ok(log_channel) = log_channel_id.to_channel(&cache_http).await else {
            continue;
        };

        let Some(log_channel) = log_channel.guild() else {
            continue;
        };

        let Ok(mut member) = message.guild_id.member(&cache_http, message.user_id).await else {
            tracing::warn!(
                "Cannot get member from user id {} in guild {}",
                message.user_id,
                message.guild_id
            );
            continue;
        };

        let timeout_end = Utc::now() + server_config.honeypot_timeout;

        match member
            .disable_communication_until_datetime(&cache_http, timeout_end.into())
            .await
        {
            Ok(_) => {
                let guild_message = format!(
                    "User {} was timed out for `{}` minutes due to posting in the honeypot channel.\nTimeout end: {}",
                    format::fdisplay(&member.user),
                    server_config.honeypot_timeout.num_minutes(),
                    format::display_time(timeout_end)
                );

                if let Err(e) = log_channel
                    .send_message(&cache_http, CreateMessage::default().content(guild_message))
                    .await
                {
                    let display_guild = match message.guild_id.to_partial_guild(&cache_http).await {
                        Ok(guild) => format::fdisplay(&guild),
                        Err(_) => message.guild_id.to_string(),
                    };

                    let log_msg = format!(
                            "Failed to inform {display_guild} in channel {} (`{}`) that a user was timed out for posting into their channel",
                            log_channel.name, log_channel.id
                        );
                    Logger::get().error(&cache_http, e, log_msg).await;
                }
            }
            Err(e) => {
                let display_guild = match message.guild_id.to_partial_guild(&cache_http).await {
                    Ok(guild) => format::fdisplay(&guild),
                    Err(_) => message.guild_id.to_string(),
                };

                let log_msg = format!(
                        "Failed to timeout user {} in guild {display_guild} after they posted a message into the honeypot channel",
                        format::display(&member.user),
                    );
                Logger::get().error(&cache_http, e, log_msg).await;
            }
        }
    }
}

async fn has_active_case(cache_http: impl CacheHttp, db_pool: &PgPool, target_user: &User) -> bool {
    if BadActorModelController::has_active_case(db_pool, target_user.id).await {
        let msg = format!(
            "User {} reached into a honeypot but already has an active case. Skipping report.",
            format::display(target_user)
        );
        Logger::get().warn(cache_http, msg).await;

        return true;
    }

    false
}

async fn save_bad_actor(
    cache_http: impl CacheHttp,
    db_pool: &PgPool,
    target_user: &User,
    options: CreateBadActorOptions,
) -> anyhow::Result<BadActor> {
    match BadActorModelController::create(db_pool, options).await {
        Ok(bad_actor) => Ok(bad_actor),
        Err(e) => {
            let log_msg = format!(
                "Failed to add bad actor {} into the database after honeypot triggered.",
                format::display(target_user)
            );
            Logger::get().error(cache_http, &e, log_msg).await;

            Err(e)
        }
    }
}

async fn get_bot_user(cache_http: impl CacheHttp, bot_id: UserId) -> anyhow::Result<User> {
    match bot_id.to_user(&cache_http).await {
        Ok(bot_user) => Ok(bot_user),
        Err(e) => {
            let log_msg = format!("Failed to get bot user from ID {bot_id}",);
            Logger::get().error(&cache_http, &e, log_msg).await;

            Err(anyhow::Error::from(e))
        }
    }
}

async fn get_origin_guild(
    cache_http: impl CacheHttp,
    origin_guild_id: GuildId,
) -> Option<PartialGuild> {
    match origin_guild_id.to_partial_guild(&cache_http).await {
        Ok(guild) => Some(guild),
        Err(e) => {
            let log_msg = format!("Failed to get guild from ID {origin_guild_id}",);
            Logger::get().error(&cache_http, &e, log_msg).await;
            None
        }
    }
}

async fn delete_msg_from_honeypot(
    cache_http: impl CacheHttp,
    cache: impl AsRef<Cache>,
    db_pool: &PgPool,
    msg: &Message,
    guild_id: GuildId,
) {
    let display_guild = match guild_id.to_partial_guild(&cache_http).await {
        Ok(guild) => format::fdisplay(&guild),
        Err(_) => guild_id.to_string(),
    };

    match msg.delete(&cache_http).await {
        Ok(_) => {
            let Some(log_channel) = get_log_channel(&cache_http, db_pool, guild_id).await else {
                return;
            };

            let embed = get_msg_deleted_embed(cache, msg);

            if let Err(e) = log_channel
                .send_message(&cache_http, CreateMessage::default().embed(embed))
                .await
            {
                let log_msg = format!(
                        "Failed to inform {display_guild} in channel {} (`{}`) that a message was deleted from their honeypot",
                        log_channel.name, log_channel.id
                    );
                Logger::get().error(&cache_http, e, log_msg).await;
            }
        }
        Err(e) => {
            let log_msg = format!(
                "Failed to delete message {} in guild {display_guild}",
                msg.id
            );
            Logger::get().error(&cache_http, e, log_msg).await;
        }
    }
}

pub async fn get_log_channel(
    cache_http: impl CacheHttp,
    db_pool: &PgPool,
    guild_id: GuildId,
) -> Option<GuildChannel> {
    let Ok(Some(server_config)) =
        ServerConfigModelController::get_by_guild_id(db_pool, guild_id).await
    else {
        return None;
    };

    let guild_channel = server_config
        .log_channel_id?
        .to_channel(&cache_http)
        .await
        .ok()?
        .guild()?;

    if !guild_channel.is_text_based() {
        return None;
    }

    Some(guild_channel)
}

fn get_msg_deleted_embed(cache: impl AsRef<Cache>, msg: &Message) -> CreateEmbed {
    let embed_author = CreateEmbedAuthor::new(msg.author.name.clone()).icon_url(
        msg.author
            .static_avatar_url()
            .unwrap_or(msg.author.default_avatar_url()),
    );

    let embed_footer = CreateEmbedFooter::new("Honeypot Log");

    let content = format!(
        "Janitor deleted a message from user {} from the honeypot channel.\n\n```{}```",
        format::fdisplay(&msg.author),
        escape_markdown(msg.content_safe(cache))
    );

    CreateEmbed::default()
        .author(embed_author)
        .title("Honeypot message deleted")
        .colour(EmbedColor::Orange)
        .description(content)
        .footer(embed_footer)
        .timestamp(Timestamp::now())
}
