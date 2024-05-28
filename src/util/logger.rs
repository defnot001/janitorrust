use std::sync::OnceLock;

use ::serenity::all::CacheHttp;
use chrono::Utc;
use poise::serenity_prelude as serenity;
use serenity::{ChannelId, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateMessage};

use crate::util::embeds::EmbedColor;

enum LogLevel {
    Warn,
    Error,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

#[derive(Debug)]
pub struct Logger {
    pub channel_id: ChannelId,
}

impl Logger {
    pub fn get() -> &'static Self {
        LOGGER
            .get()
            .expect("Logger should have been initialized by now!")
    }

    pub fn set(channel_id: ChannelId) {
        LOGGER
            .set(Logger { channel_id })
            .expect("Failed to set logger!");
    }

    pub async fn warn(&self, cache_http: impl CacheHttp, msg: impl AsRef<str>) {
        let msg = sanitize_msg(msg.as_ref());
        tracing::warn!("{msg}");

        let embed = Self::log_embed::<i32>(msg, LogLevel::Warn, None).await;

        if let Err(e) = self
            .channel_id
            .send_message(cache_http, CreateMessage::default().add_embed(embed))
            .await
        {
            tracing::error!("Failed to send warn log embed to channel: {e}");
        }
    }

    pub async fn error(
        &self,
        cache_http: impl CacheHttp,
        e: impl std::fmt::Display,
        log_msg: impl AsRef<str>,
    ) {
        let msg = sanitize_msg(log_msg.as_ref());
        tracing::error!("{msg}: {e}");

        let embed = Self::log_embed(msg, LogLevel::Error, Some(e)).await;

        if let Err(e) = self
            .channel_id
            .send_message(cache_http, CreateMessage::default().add_embed(embed))
            .await
        {
            tracing::error!("Failed to send error log embed to channel: {e}");
        }
    }

    async fn log_embed<E>(msg: &str, log_level: LogLevel, error: Option<E>) -> CreateEmbed
    where
        E: std::fmt::Display,
    {
        let embed_color = match log_level {
            LogLevel::Warn => EmbedColor::Yellow,
            LogLevel::Error => EmbedColor::Red,
        };

        let description = match error {
            Some(e) => format!("{msg}\n\n```{e}```"),
            None => msg.to_string(),
        };

        let embed_author = CreateEmbedAuthor::new("Janitor");
        let embed_footer = CreateEmbedFooter::new("Error Log");

        CreateEmbed::default()
            .description(description)
            .color(embed_color)
            .author(embed_author)
            .footer(embed_footer)
            .timestamp(Utc::now())
    }
}

pub fn sanitize_msg(msg: &str) -> &str {
    if msg.ends_with('.') || msg.ends_with('!') {
        let Some((i, _)) = msg.char_indices().last() else {
            return msg;
        };

        msg[..i].trim_end()
    } else {
        msg
    }
}
