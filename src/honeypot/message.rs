use std::sync::Arc;
use std::time::{Duration, Instant};

use poise::{serenity_prelude as serenity, FrameworkContext};
use serenity::{Context, GuildId, Message, UserId};
use tokio::sync::{Mutex, MutexGuard};

// use crate::broadcast::broadcast_handler::BroadcastOptions;
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
        // report_bad_actor().await;

        // let options = BroadcastOptions {
        //     ctx: todo!(),
        //     target_user: todo!(),
        //     bad_actor: todo!(),
        //     interaction_guild: todo!(),
        //     broadcast_type: todo!(),
        // }
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
