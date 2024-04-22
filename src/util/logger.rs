use std::sync::OnceLock;

use anyhow::Context;
use chrono::Utc;
use serenity::all::{
    ChannelId, ChannelType, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateMessage,
};

use crate::Context as AppContext;

use super::builders::EmbedColor;

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
        LOGGER.set(Logger { channel_id });
    }

    pub async fn warn(&self, ctx: &AppContext<'_>, msg: impl AsRef<str>) {
        let msg = sanitize_msg(msg.as_ref());
        tracing::warn!(msg);

        let embed = Self::log_embed::<i32>(msg, ctx, LogLevel::Warn, None).await;

        if let Err(e) = self
            .channel_id
            .send_message(&ctx, CreateMessage::default().add_embed(embed))
            .await
        {
            tracing::error!("Failed to send warn log embed to channel: {e}");
        }
    }

    pub async fn error(
        &self,
        ctx: &AppContext<'_>,
        e: impl std::fmt::Display,
        log_msg: impl AsRef<str>,
    ) {
        let msg = sanitize_msg(log_msg.as_ref());
        tracing::error!("{msg}: {e}");

        let embed = Self::log_embed(msg, ctx, LogLevel::Warn, Some(e)).await;

        if let Err(e) = self
            .channel_id
            .send_message(&ctx, CreateMessage::default().add_embed(embed))
            .await
        {
            tracing::error!("Failed to send error log embed to channel: {e}");
        }
    }

    async fn log_embed<E>(
        msg: &str,
        ctx: &AppContext<'_>,
        log_level: LogLevel,
        error: Option<E>,
    ) -> CreateEmbed
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

        let embed_author = match ctx.framework().bot_id.to_user(&ctx).await {
            Ok(user) => {
                let avatar_url = user
                    .static_avatar_url()
                    .unwrap_or(user.default_avatar_url());
                CreateEmbedAuthor::new("Janitor").icon_url(avatar_url)
            }
            Err(e) => {
                tracing::error!("Failed to fetch bot user: {e}");
                CreateEmbedAuthor::new("Janitor")
            }
        };

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
