use std::sync::Arc;
use std::time::{Duration, Instant};

use poise::{serenity_prelude as serenity, FrameworkContext};
use serenity::{Context, GuildId, Message, UserId};
use tokio::sync::{Mutex, MutexGuard};

use crate::broadcast::broadcast_handler::{broadcast, BroadcastOptions, BroadcastType};
use crate::database::controllers::badactor_model_controller::{
    BadActorModelController, BadActorType, CreateBadActorOptions,
};
use crate::util::format;
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

    let is_in_honeypot = framework
        .user_data
        .honeypot_channels
        .contains(&msg.channel_id);

    if is_in_honeypot {
        if let Err(e) = msg.delete(ctx).await {
            let display_guild = if let Ok(guild) = guild_id.to_partial_guild(ctx).await {
                format::fdisplay(&guild)
            } else {
                guild_id.to_string()
            };

            let log_msg = format!(
                "Failed to delete message {} in guild {display_guild}",
                msg.id
            );
            Logger::get().error(ctx, e, log_msg).await;
        }
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
        let bad_actor_options = CreateBadActorOptions {
            user_id: msg.author.id,
            actor_type: BadActorType::Honeypot,
            screenshot_proof: None,
            explanation: Some("reached into the honeypot".to_string()),
            origin_guild_id: guild_id,
            updated_by_user_id: framework.bot_id,
        };

        let bad_actor =
            match BadActorModelController::create(&framework.user_data.db_pool, bad_actor_options)
                .await
            {
                Ok(bad_actor) => bad_actor,
                Err(e) => {
                    let log_msg = format!(
                        "Failed to add bad actor {} into the database after honeypot triggered.",
                        msg.author.id
                    );
                    Logger::get().error(&ctx, e, log_msg).await;
                    return;
                }
            };

        let bot_user = match framework.bot_id.to_user(&ctx).await {
            Ok(bot_user) => bot_user,
            Err(e) => {
                let log_msg = "Failed to get bot user from the API";
                Logger::get().error(ctx, e, log_msg).await;
                return;
            }
        };

        let broadcast_options = BroadcastOptions {
            config: &framework.user_data.config,
            db_pool: &framework.user_data.db_pool,
            reporting_user: &bot_user,
            reporting_bot_id: bot_user.id,
            bad_actor: &bad_actor,
            bad_actor_user: &msg.author,
            origin_guild: &None,
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
