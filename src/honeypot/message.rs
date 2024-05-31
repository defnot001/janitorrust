use std::sync::Arc;
use std::time::{Duration, Instant};

use poise::{serenity_prelude as serenity, FrameworkContext};
use serenity::{
    Cache, CacheHttp, Context, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateMessage,
    GuildChannel, GuildId, Message, PartialGuild, Timestamp, User, UserId,
};
use sqlx::PgPool;
use tokio::sync::{Mutex, MutexGuard};

use crate::broadcast::broadcast_handler::{broadcast, BroadcastOptions, BroadcastType};
use crate::database::controllers::badactor_model_controller::{
    BadActor, BadActorModelController, BadActorType, CreateBadActorOptions,
};
use crate::database::controllers::serverconfig_model_controller::ServerConfigModelController;
use crate::util::embeds::EmbedColor;
use crate::util::format::{self, escape_markdown};
use crate::util::logger::Logger;
use crate::Data;

pub type Queue = Arc<Mutex<Vec<HoneypotMessage>>>;

#[derive(Debug)]
pub struct HoneypotMessage {
    pub guild_id: GuildId,
    pub user_id: UserId,
    pub content: String,
    pub timestamp: Instant,
    pub is_in_honeypot: bool,
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

    remove_old_messages(&mut queue, now);

    let honeypot_msg = HoneypotMessage {
        guild_id,
        user_id: msg.author.id,
        content: msg.content.clone(),
        is_in_honeypot,
        timestamp: now,
    };

    let should_report = should_report(&queue, &honeypot_msg);
    queue.push(honeypot_msg);

    // drop the MutexGuard which unlocks the mutex again
    drop(queue);

    if should_report {
        if has_active_case(ctx, &framework.user_data.db_pool, &msg.author).await {
            return;
        }

        let bad_actor_options = CreateBadActorOptions {
            user_id: msg.author.id,
            actor_type: BadActorType::Honeypot,
            screenshot_proof: None,
            explanation: Some("reached into the honeypot".to_string()),
            origin_guild_id: guild_id,
            updated_by_user_id: framework.bot_id,
        };

        let bad_actor_future = save_bad_actor(
            ctx,
            &framework.user_data.db_pool,
            &msg.author,
            bad_actor_options,
        );

        let bot_user_future = get_bot_user(ctx, framework.bot_id);
        let origin_guild_future = get_origin_guild(ctx, guild_id);

        let (bad_actor, bot_user, origin_guild) =
            tokio::join!(bad_actor_future, bot_user_future, origin_guild_future);

        let Ok(bot_user) = bot_user else {
            return;
        };

        let Ok(bad_actor) = bad_actor else {
            return;
        };

        let broadcast_options = BroadcastOptions {
            config: &framework.user_data.config,
            db_pool: &framework.user_data.db_pool,
            reporting_user: &bot_user,
            reporting_bot_id: bot_user.id,
            bad_actor: &bad_actor,
            bad_actor_user: &msg.author,
            origin_guild,
            origin_guild_id: guild_id,
            broadcast_type: BroadcastType::Honeypot,
        };

        broadcast(&ctx, broadcast_options).await;
    }
}

fn remove_old_messages(queue: &mut MutexGuard<'_, Vec<HoneypotMessage>>, now: Instant) {
    let first_new_msg = queue
        .iter()
        .enumerate()
        .find(|(_, msg)| now - msg.timestamp < Duration::from_secs(60))
        .map(|(i, _)| i)
        .unwrap_or(queue.len());

    queue.drain(..first_new_msg);
}

fn should_report(queue: &MutexGuard<'_, Vec<HoneypotMessage>>, new_msg: &HoneypotMessage) -> bool {
    let mut equal_msg_content: usize = 0;
    let mut is_any_in_honeypot = new_msg.is_in_honeypot;

    for queue_msg in queue.iter() {
        if queue_msg.user_id == new_msg.user_id && queue_msg.content == new_msg.content {
            equal_msg_content += 1;
            is_any_in_honeypot |= queue_msg.is_in_honeypot;
        }
    }

    equal_msg_content >= 2 && is_any_in_honeypot
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
            return Err(e);
        }
    }
}

async fn get_bot_user(cache_http: impl CacheHttp, bot_id: UserId) -> anyhow::Result<User> {
    match bot_id.to_user(&cache_http).await {
        Ok(bot_user) => Ok(bot_user),
        Err(e) => {
            let log_msg = format!("Failed to get bot user from ID {bot_id}",);
            Logger::get().error(&cache_http, &e, log_msg).await;
            return Err(anyhow::Error::from(e));
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

    let Some(log_channel_id) = server_config.log_channel_id else {
        return None;
    };

    let Ok(log_channel) = log_channel_id.to_channel(&cache_http).await else {
        return None;
    };

    let Some(guild_channel) = log_channel.guild() else {
        return None;
    };

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
