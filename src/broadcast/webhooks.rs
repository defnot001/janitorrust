use std::str::FromStr;

use anyhow::Context;
use futures::future;
use poise::serenity_prelude as serenity;
use serenity::{CacheHttp, CreateAttachment, CreateEmbed, ExecuteWebhook, GuildId, Webhook};
use sqlx::prelude::FromRow;
use sqlx::PgPool;
use url::Url;

use crate::util::format;
use crate::util::logger::Logger;

use super::broadcast_handler::BroadcastType;

#[derive(Debug, FromRow)]
struct DbBroadcastWebhook {
    guild_id: String,
    guild_name: String,
    webhook_url: String,
}

#[derive(Debug)]
struct BroadcastWebhook {
    guild_id: GuildId,
    guild_name: String,
    webhook_url: Url,
}

impl TryFrom<DbBroadcastWebhook> for BroadcastWebhook {
    type Error = anyhow::Error;

    fn try_from(db_webhook: DbBroadcastWebhook) -> Result<Self, Self::Error> {
        let guild_id = GuildId::from_str(&db_webhook.guild_id)?;
        let webhook_url = Url::from_str(&db_webhook.webhook_url)?;

        Ok(Self {
            guild_id,
            webhook_url,
            guild_name: db_webhook.guild_name,
        })
    }
}

#[derive(Debug)]
struct WebhookListenerResult {
    guild_id: GuildId,
    guild_name: String,
    webhook: anyhow::Result<Webhook>,
}

#[derive(Debug)]
struct WebhookListener {
    guild_id: GuildId,
    guild_name: String,
    webhook: Webhook,
}

pub struct BroadcastWebhookOptions<'a> {
    pub db_pool: &'a PgPool,
    pub broadcast_type: BroadcastType,
    pub embed: &'a CreateEmbed,
    pub attachment: &'a Option<CreateAttachment>,
}

pub async fn broadcast_to_webhooks(
    cache_http: impl CacheHttp,
    options: BroadcastWebhookOptions<'_>,
) {
    let BroadcastWebhookOptions {
        db_pool,
        broadcast_type,
        embed,
        attachment,
    } = options;

    let webhooks = match get_webhooks_from_db(db_pool).await {
        Ok(webhooks) => webhooks,
        Err(e) => {
            let log_msg = "Failed to get webhooks to broadcast to from the database";
            Logger::get().error(&cache_http, e, log_msg).await;

            return;
        }
    };

    let webhooks = get_discord_webhooks(&cache_http, webhooks).await;

    let futures = webhooks.into_iter().map(|l| {
        let execute = if let Some(attachment) = attachment.clone() {
            ExecuteWebhook::default()
                .content(broadcast_type.message())
                .embed(embed.clone())
                .add_file(attachment)
        } else {
            ExecuteWebhook::default()
                .content(broadcast_type.message())
                .embed(embed.clone())
        };

        let http = cache_http.http();

        async move {
            if let Err(e) = l.webhook.execute(http, false, execute).await {
                let log_msg = format!(
                    "Failed to send broadcast embed to webhook in guild {} ({})",
                    l.guild_name, l.guild_id
                );
                Logger::get().error(http, e, log_msg).await;
            }
        }
    });

    future::join_all(futures).await;
}

async fn get_webhooks_from_db(db_pool: &PgPool) -> anyhow::Result<Vec<BroadcastWebhook>> {
    sqlx::query_as::<_, DbBroadcastWebhook>("SELECT * FROM webhooks;")
        .fetch_all(db_pool)
        .await
        .context("Failed to get all broadcast webhooks from the `webhooks` table")?
        .into_iter()
        .map(BroadcastWebhook::try_from)
        .collect::<anyhow::Result<Vec<_>>>()
}

async fn get_discord_webhooks(
    cache_http: impl CacheHttp,
    webhooks: Vec<BroadcastWebhook>,
) -> Vec<WebhookListener> {
    let len = webhooks.len();
    let http = cache_http.http();

    let iter = webhooks.into_iter().map(|w| async move {
        let webhook = Webhook::from_url(http, w.webhook_url.as_str())
            .await
            .map_err(anyhow::Error::from);

        WebhookListenerResult {
            guild_id: w.guild_id,
            guild_name: w.guild_name,
            webhook,
        }
    });

    let results = future::join_all(iter).await;

    let mut good_webhooks = Vec::with_capacity(len);

    for listener in results {
        match listener.webhook {
            Ok(webhook) => {
                let listener = WebhookListener {
                    guild_id: listener.guild_id,
                    guild_name: listener.guild_name,
                    webhook,
                };

                good_webhooks.push(listener);
            }
            Err(e) => {
                let logger = Logger::get();

                if let Ok(guild) = listener.guild_id.to_partial_guild(&cache_http).await {
                    let log_msg = format!(
                        "Failed to connect to webhook in guild {}",
                        format::display(&guild)
                    );

                    logger.error(&cache_http, e, log_msg).await;
                } else {
                    let log_msg = format!(
                        "Failed to connect to webhook in guild {} ({})",
                        listener.guild_name, listener.guild_id
                    );

                    logger.error(&cache_http, e, log_msg).await;
                }
            }
        }
    }

    good_webhooks
}
