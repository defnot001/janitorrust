use std::{fmt::Display, str::FromStr};

use futures::TryFutureExt;
use poise::serenity_prelude as serenity;
use serenity::{
    CacheHttp, ComponentInteraction, ComponentInteractionDataKind, CreateMessage, EditMessage,
    Embed, GuildChannel, GuildId, User, UserId,
};
use sqlx::PgPool;

use crate::{
    honeypot::message::get_log_channel,
    util::{format, logger::Logger},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CustomId {
    Ban,
    SoftBan,
    Kick,
    Unban,
    Confirm,
    Cancel,
}

impl FromStr for CustomId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ban" => Ok(Self::Ban),
            "softban" => Ok(Self::SoftBan),
            "kick" => Ok(Self::Kick),
            "unban" => Ok(Self::Unban),
            "confirm" => Ok(Self::Confirm),
            "cancel" => Ok(Self::Cancel),
            _ => anyhow::bail!("Unknown custom id {s}"),
        }
    }
}

impl Display for CustomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ban => write!(f, "ban"),
            Self::SoftBan => write!(f, "softban"),
            Self::Kick => write!(f, "kick"),
            Self::Unban => write!(f, "unban"),
            Self::Confirm => write!(f, "confirm"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}

pub async fn handle_component_interaction(
    interaction: &ComponentInteraction,
    cache_http: impl CacheHttp,
    db_pool: &PgPool,
) -> anyhow::Result<()> {
    match interaction.data.kind {
        ComponentInteractionDataKind::Button => {
            handle_button(interaction, &cache_http, db_pool).await?;
        }
        _ => return Ok(()),
    }

    Ok(())
}

async fn handle_button(
    interaction: &ComponentInteraction,
    cache_http: impl CacheHttp,
    db_pool: &PgPool,
) -> anyhow::Result<()> {
    let Some(interaction_guild_id) = interaction.guild_id else {
        return Ok(());
    };

    let custom_id = CustomId::from_str(&interaction.data.custom_id)?;

    if custom_id == CustomId::Confirm || custom_id == CustomId::Cancel {
        return Ok(());
    }

    let Some(embed) = get_broadcast_embed(interaction) else {
        return Ok(());
    };

    let interaction_user = interaction.user.clone();

    let Ok(interaction_member) = interaction_guild_id
        .member(&cache_http, interaction_user.id)
        .await
    else {
        return Ok(());
    };

    let Some(permissions) = interaction_member.permissions else {
        let message = format!(
            "Cannot get permissions for member {}",
            format::display(&interaction_user)
        );
        tracing::warn!("{message}");
        return Ok(());
    };

    match custom_id {
        CustomId::Ban | CustomId::SoftBan | CustomId::Unban => {
            if !permissions.ban_members() {
                return Ok(());
            }
        }
        CustomId::Kick => {
            if !permissions.kick_members() {
                return Ok(());
            }
        }
    }

    if !permissions.ban_members() {
        return Ok(());
    }

    let target_user = get_target_user(&embed, &cache_http).await?;

    handle_moderation(
        &cache_http,
        db_pool,
        interaction_guild_id,
        &target_user,
        &interaction_user,
        custom_id,
    )
    .await;

    if cache_http.cache().is_none() {
        let log_msg = "Failed to get cache to request broadcast message content";
        Logger::get().warn(&cache_http, log_msg).await;
    }

    if let Err(e) = interaction
        .message
        .clone()
        .edit(&cache_http, EditMessage::new().components(vec![]))
        .await
    {
        let display_guild = match interaction_guild_id.to_partial_guild(&cache_http).await {
            Ok(g) => format::fdisplay(&g),
            Err(_) => interaction_guild_id.to_string(),
        };

        let log_msg = format!(
            "Failed to remove buttons from broadcast embed for target user {} in {display_guild}",
            format::display(&target_user)
        );
        Logger::get().error(&cache_http, e, log_msg).await;
    }

    Ok(())
}

fn get_broadcast_embed(interaction: &ComponentInteraction) -> Option<Embed> {
    let embeds = interaction.message.embeds.clone();

    let Some(first_embed) = embeds.into_iter().next() else {
        return None;
    };

    let fields_len = first_embed.fields.len();

    let expected_field_names = [
        "Report ID",
        "Active",
        "Type",
        "Explanation",
        "Server of Origin",
        "Last Updated By",
    ];

    if fields_len != expected_field_names.len() {
        return None;
    }

    for field in &first_embed.fields {
        if !expected_field_names.contains(&field.name.as_str()) {
            return None;
        }
    }

    Some(first_embed)
}

async fn get_target_user(
    broadcast_embed: &Embed,
    cache_http: impl CacheHttp,
) -> anyhow::Result<User> {
    let Some(title) = broadcast_embed.title.clone() else {
        anyhow::bail!("Broadcast embed missing title");
    };

    let Some(id_start) = title.find('`') else {
        anyhow::bail!(
            "Failed to find first backtick for parsing the userid from broadcast embed title"
        );
    };
    let Some(id_end) = title[id_start + 1..].find('`') else {
        anyhow::bail!(
            "Failed to find last backtick for parsing the userid from broadcast embed title"
        );
    };

    let id_str = &title[id_start + 1..id_start + 1 + id_end];
    let user_id = UserId::from_str(id_str)?;

    user_id
        .to_user(cache_http)
        .await
        .map_err(anyhow::Error::from)
}

pub async fn handle_moderation(
    cache_http: impl CacheHttp,
    db_pool: &PgPool,
    interaction_guild_id: GuildId,
    target_user: &User,
    interaction_user: &User,
    custom_id: CustomId,
) {
    let Some(log_channel) = get_log_channel(&cache_http, db_pool, interaction_guild_id).await
    else {
        let display_guild = match interaction_guild_id.to_partial_guild(&cache_http).await {
            Ok(g) => format::fdisplay(&g),
            Err(_) => interaction_guild_id.to_string(),
        };

        let log_msg = format!(
            "Cannot moderate {} in guild {display_guild} because of missing log channel",
            format::display(target_user)
        );
        Logger::get().warn(&cache_http, log_msg).await;

        return;
    };

    match custom_id {
        CustomId::Ban => {
            let ban_res = interaction_guild_id
                .ban(&cache_http.http(), target_user.id, 7)
                .await;

            if let Err(e) = ban_res {
                handle_moderation_fail(
                    &cache_http,
                    &log_channel,
                    anyhow::Error::from(e),
                    custom_id,
                    target_user,
                    interaction_guild_id,
                )
                .await;
            } else {
                handle_moderation_success(
                    &cache_http,
                    &log_channel,
                    custom_id,
                    target_user,
                    interaction_user,
                    interaction_guild_id,
                )
                .await;
            }
        }
        CustomId::SoftBan => {
            let http = cache_http.http();

            let softban_res = interaction_guild_id
                .ban(http, target_user.id, 7)
                .and_then(|_| async move { interaction_guild_id.unban(http, target_user.id).await })
                .await;

            if let Err(e) = softban_res {
                handle_moderation_fail(
                    &cache_http,
                    &log_channel,
                    anyhow::Error::from(e),
                    custom_id,
                    target_user,
                    interaction_guild_id,
                )
                .await;
            } else {
                handle_moderation_success(
                    &cache_http,
                    &log_channel,
                    custom_id,
                    target_user,
                    interaction_user,
                    interaction_guild_id,
                )
                .await;
            }
        }
        CustomId::Kick => {
            let kick_res = interaction_guild_id
                .kick(&cache_http.http(), target_user.id)
                .await;

            if let Err(e) = kick_res {
                handle_moderation_fail(
                    &cache_http,
                    &log_channel,
                    anyhow::Error::from(e),
                    custom_id,
                    target_user,
                    interaction_guild_id,
                )
                .await;
            } else {
                handle_moderation_success(
                    &cache_http,
                    &log_channel,
                    custom_id,
                    target_user,
                    interaction_user,
                    interaction_guild_id,
                )
                .await;
            }
        }
        CustomId::Unban => {
            let unban_res = interaction_guild_id
                .unban(&cache_http.http(), target_user.id)
                .await;

            if let Err(e) = unban_res {
                handle_moderation_fail(
                    &cache_http,
                    &log_channel,
                    anyhow::Error::from(e),
                    custom_id,
                    target_user,
                    interaction_guild_id,
                )
                .await;
            } else {
                handle_moderation_success(
                    &cache_http,
                    &log_channel,
                    custom_id,
                    target_user,
                    interaction_user,
                    interaction_guild_id,
                )
                .await;
            }
        }
    }
}

async fn handle_moderation_fail(
    cache_http: impl CacheHttp,
    log_channel: &GuildChannel,
    e: anyhow::Error,
    custom_id: CustomId,
    target_user: &User,
    interaction_guild_id: GuildId,
) {
    let display_guild = match interaction_guild_id.to_partial_guild(&cache_http).await {
        Ok(g) => format::fdisplay(&g),
        Err(_) => interaction_guild_id.to_string(),
    };

    let log_msg = format!(
        "Failed to {custom_id} user {} from {display_guild}",
        format::display(target_user)
    );
    Logger::get().error(&cache_http, e, log_msg).await;

    let guild_message = format!(
        "Failed to {custom_id} user {} from your guild!",
        format::fdisplay(target_user)
    );

    if let Err(e) = log_channel
        .send_message(&cache_http, CreateMessage::default().content(guild_message))
        .await
    {
        let log_msg = format!(
            "Failed to inform guild {display_guild} that moderation action `{custom_id}ì for target user {} using the broadcast embed buttons failed",
            format::fdisplay(target_user)
        );
        Logger::get().error(&cache_http, e, log_msg).await;
    }
}

async fn handle_moderation_success(
    cache_http: impl CacheHttp,
    log_channel: &GuildChannel,
    custom_id: CustomId,
    target_user: &User,
    interaction_user: &User,
    interaction_guild_id: GuildId,
) {
    let guild_message = format!(
        "{} took moderation action `{custom_id} against user {} using the broadcast embed buttons.`",
        format::fdisplay(interaction_user),
        format::fdisplay(target_user)
    );

    if let Err(e) = log_channel
        .send_message(&cache_http, CreateMessage::default().content(guild_message))
        .await
    {
        let display_guild = match interaction_guild_id.to_partial_guild(&cache_http).await {
            Ok(g) => format::fdisplay(&g),
            Err(_) => interaction_guild_id.to_string(),
        };

        let log_msg = format!(
            "Failed to inform guild {display_guild} that moderation action {custom_id} was successfully performed against user {} using the broadcast embed buttons",
            format::fdisplay(target_user)
        );
        Logger::get().error(&cache_http, e, log_msg).await;
    }
}
