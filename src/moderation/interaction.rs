use std::{fmt::Display, str::FromStr};

use futures::TryFutureExt;
use poise::serenity_prelude as serenity;
use serenity::{
    CacheHttp, ComponentInteraction, ComponentInteractionDataKind, CreateMessage, EditMessage,
    Embed, GuildChannel, GuildId, Member, Message, User, UserId,
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
    NoAction,
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
            "no_action" => Ok(Self::NoAction),
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
            Self::NoAction => write!(f, "no_action"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModerationCustomId {
    Ban,
    SoftBan,
    Kick,
    Unban,
    NoAction,
}

impl Display for ModerationCustomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ban => write!(f, "ban"),
            Self::SoftBan => write!(f, "softban"),
            Self::Kick => write!(f, "kick"),
            Self::Unban => write!(f, "unban"),
            Self::NoAction => write!(f, "no_action"),
        }
    }
}

impl TryFrom<CustomId> for ModerationCustomId {
    type Error = anyhow::Error;

    fn try_from(custom_id: CustomId) -> Result<Self, Self::Error> {
        match custom_id {
            CustomId::Ban => Ok(ModerationCustomId::Ban),
            CustomId::SoftBan => Ok(ModerationCustomId::SoftBan),
            CustomId::Kick => Ok(ModerationCustomId::Kick),
            CustomId::Unban => Ok(ModerationCustomId::Unban),
            CustomId::NoAction => Ok(ModerationCustomId::NoAction),
            _ => anyhow::bail!("custom id `{custom_id}` is not a custom moderation id."),
        }
    }
}

#[derive(Debug)]
pub struct HandleModerationOptions<'a> {
    interaction_guild_id: GuildId,
    custom_id: ModerationCustomId,
    db_pool: &'a PgPool,
    target_user: &'a User,
    interaction_user: &'a User,
    embed: &'a Embed,
}

#[derive(Debug)]
struct RemoveButtonOptions<'a> {
    interaction_guild_id: GuildId,
    target_user: &'a User,
    message: &'a mut Box<Message>,
}

#[derive(Debug)]
struct CanModerateOptions<'a> {
    interaction_guild_id: GuildId,
    custom_id: ModerationCustomId,
    interaction_member: &'a Member,
}

#[derive(Debug)]
struct HandleModerationFailOptions<'a> {
    error: anyhow::Error,
    custom_id: ModerationCustomId,
    interaction_guild_id: GuildId,
    log_channel: &'a GuildChannel,
    target_user: &'a User,
}

#[derive(Debug)]
struct HandleModerationSuccessOptions<'a> {
    custom_id: ModerationCustomId,
    interaction_guild_id: GuildId,
    log_channel: &'a GuildChannel,
    target_user: &'a User,
    interaction_user: &'a User,
}

pub async fn handle_component_interaction(
    interaction: &ComponentInteraction,
    cache_http: impl CacheHttp,
    db_pool: &PgPool,
) -> anyhow::Result<()> {
    match interaction.data.kind {
        ComponentInteractionDataKind::Button => {
            handle_button_interaction(interaction, &cache_http, db_pool).await?;
        }
        _ => return Ok(()),
    }

    Ok(())
}

async fn handle_button_interaction(
    interaction: &ComponentInteraction,
    cache_http: impl CacheHttp,
    db_pool: &PgPool,
) -> anyhow::Result<()> {
    let Some(interaction_guild_id) = interaction.guild_id else {
        return Ok(());
    };

    let Ok(custom_id) =
        ModerationCustomId::try_from(CustomId::from_str(&interaction.data.custom_id)?)
    else {
        return Ok(());
    };

    let Some(embed) = get_broadcast_embed(interaction) else {
        return Ok(());
    };

    let Ok(interaction_member) = interaction_guild_id
        .member(&cache_http, interaction.user.id)
        .await
    else {
        return Ok(());
    };

    let options = CanModerateOptions {
        interaction_guild_id,
        custom_id,
        interaction_member: &interaction_member,
    };
    if !can_moderate(&cache_http, options).await {
        return Ok(());
    }

    let target_user = get_target_user(&cache_http, &embed).await?;

    let options = HandleModerationOptions {
        interaction_guild_id,
        custom_id,
        db_pool,
        target_user: &target_user,
        interaction_user: &interaction_member.user,
        embed: &embed,
    };
    handle_moderation(&cache_http, options).await;

    let options = RemoveButtonOptions {
        interaction_guild_id,
        target_user: &target_user,
        message: &mut interaction.message.clone(),
    };
    remove_buttons(&cache_http, options).await;

    Ok(())
}

async fn remove_buttons(cache_http: impl CacheHttp, options: RemoveButtonOptions<'_>) {
    let RemoveButtonOptions {
        interaction_guild_id,
        target_user,
        message,
    } = options;

    if let Err(e) = message
        .edit(&cache_http, EditMessage::new().components(vec![]))
        .await
    {
        let display_guild = match interaction_guild_id.to_partial_guild(&cache_http).await {
            Ok(g) => format::fdisplay(&g),
            Err(_) => interaction_guild_id.to_string(),
        };

        let log_msg = format!(
            "Failed to remove buttons from broadcast embed for target user {} in {display_guild}",
            format::display(target_user)
        );
        Logger::get().error(&cache_http, e, log_msg).await;
    }
}

async fn can_moderate(cache_http: impl CacheHttp, options: CanModerateOptions<'_>) -> bool {
    let CanModerateOptions {
        interaction_guild_id,
        custom_id,
        interaction_member,
    } = options;

    let Some(cache) = cache_http.cache() else {
        tracing::warn!("Failed to get bot cache in button interaction handler");
        return false;
    };

    let permissions = match interaction_member.permissions(cache) {
        Ok(permissions) => permissions,
        Err(e) => {
            let display_guild = match interaction_guild_id.to_partial_guild(&cache_http).await {
                Ok(g) => format::fdisplay(&g),
                Err(_) => interaction_guild_id.to_string(),
            };

            let log_msg = format!(
                "Failed to get guild level permissions for user {} in guild {display_guild}",
                format::display(&interaction_member.user)
            );
            Logger::get().error(&cache_http, e, log_msg).await;

            return false;
        }
    };

    match custom_id {
        ModerationCustomId::Ban
        | ModerationCustomId::SoftBan
        | ModerationCustomId::Unban
        | ModerationCustomId::NoAction => {
            if !permissions.ban_members() {
                let message = format!("Guild member {} tried to use moderation button `{custom_id}` but lacks ban permissions", format::display(&interaction_member.user));
                tracing::warn!("{message}");

                return false;
            }
        }
        ModerationCustomId::Kick => {
            if !permissions.kick_members() {
                let message = format!("Guild member {} tried to use moderation button `{custom_id}` but lacks kick permissions", format::display(&interaction_member.user));
                tracing::warn!("{message}");

                return false;
            }
        }
    }

    true
}

fn get_broadcast_embed(interaction: &ComponentInteraction) -> Option<Embed> {
    let embeds = interaction.message.embeds.clone();

    let first_embed = embeds.into_iter().next()?;
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
    cache_http: impl CacheHttp,
    broadcast_embed: &Embed,
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

fn get_ban_reason(embed: &Embed) -> anyhow::Result<String> {
    let embed_fields = embed.fields.clone();

    let Some(report_id_field) = embed_fields.iter().find(|f| f.name.as_str() == "Report ID") else {
        anyhow::bail!("Cannot find field `Report ID` in broadcast embed")
    };

    let Some(type_field) = embed_fields.iter().find(|f| f.name.as_str() == "Type") else {
        anyhow::bail!("Cannot find field `Type` in broadcast embed")
    };

    Ok(format!(
        "Bad Actor {} ({})",
        type_field.value, report_id_field.value
    ))
}

pub async fn handle_moderation(cache_http: impl CacheHttp, options: HandleModerationOptions<'_>) {
    let HandleModerationOptions {
        interaction_guild_id,
        custom_id,
        db_pool,
        target_user,
        interaction_user,
        embed,
    } = options;

    if custom_id == ModerationCustomId::NoAction {
        return;
    }

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

    let mut moderation_error = None;

    match custom_id {
        ModerationCustomId::Ban => {
            if let Ok(ban_reason) = get_ban_reason(embed) {
                if let Err(e) = interaction_guild_id
                    .ban_with_reason(&cache_http.http(), target_user.id, 7, ban_reason)
                    .await
                {
                    moderation_error = Some(anyhow::Error::from(e));
                }
            } else if let Err(e) = interaction_guild_id
                .ban(&cache_http.http(), target_user.id, 7)
                .await
            {
                moderation_error = Some(anyhow::Error::from(e));
            }
        }
        ModerationCustomId::SoftBan => {
            let http = cache_http.http();

            let softban_res = interaction_guild_id
                .ban(http, target_user.id, 7)
                .and_then(|_| async move { interaction_guild_id.unban(http, target_user.id).await })
                .await;

            if let Err(e) = softban_res {
                moderation_error = Some(anyhow::Error::from(e));
            }
        }
        ModerationCustomId::Kick => {
            if let Err(e) = interaction_guild_id
                .kick(&cache_http.http(), target_user.id)
                .await
            {
                moderation_error = Some(anyhow::Error::from(e));
            }
        }
        ModerationCustomId::Unban => {
            if let Err(e) = interaction_guild_id
                .unban(&cache_http.http(), target_user.id)
                .await
            {
                if e.to_string() == "Unknown Ban" {
                    return handle_unknown_ban(
                        &cache_http,
                        interaction_guild_id,
                        &log_channel,
                        target_user,
                    )
                    .await;
                } else {
                    moderation_error = Some(anyhow::Error::from(e));
                }
            }
        }
        ModerationCustomId::NoAction => {
            // Safety: The guard clause at the beginning of this function returns early!
            unreachable!()
        }
    }

    if let Some(e) = moderation_error {
        let options = HandleModerationFailOptions {
            custom_id,
            interaction_guild_id,
            target_user,
            error: e,
            log_channel: &log_channel,
        };
        handle_moderation_fail(&cache_http, options).await;
    } else {
        let options = HandleModerationSuccessOptions {
            custom_id,
            interaction_guild_id,
            target_user,
            interaction_user,
            log_channel: &log_channel,
        };
        handle_moderation_success(&cache_http, options).await;
    }
}

async fn handle_moderation_fail(
    cache_http: impl CacheHttp,
    options: HandleModerationFailOptions<'_>,
) {
    let HandleModerationFailOptions {
        error,
        custom_id,
        interaction_guild_id,
        log_channel,
        target_user,
    } = options;

    let display_guild = match interaction_guild_id.to_partial_guild(&cache_http).await {
        Ok(g) => format::fdisplay(&g),
        Err(_) => interaction_guild_id.to_string(),
    };

    let log_msg = format!(
        "Failed to {custom_id} user {} from {display_guild}",
        format::display(target_user)
    );
    Logger::get().error(&cache_http, error, log_msg).await;

    let guild_message = format!(
        "Failed to {custom_id} user {} from your guild!",
        format::fdisplay(target_user)
    );

    if let Err(e) = log_channel
        .send_message(&cache_http, CreateMessage::default().content(guild_message))
        .await
    {
        let log_msg = format!(
            "Failed to inform guild {display_guild} that moderation action `{custom_id} for target user {} using the broadcast embed buttons failed",
            format::fdisplay(target_user)
        );
        Logger::get().error(&cache_http, e, log_msg).await;
    }
}

async fn handle_moderation_success(
    cache_http: impl CacheHttp,
    options: HandleModerationSuccessOptions<'_>,
) {
    let HandleModerationSuccessOptions {
        custom_id,
        interaction_guild_id,
        log_channel,
        target_user,
        interaction_user,
    } = options;

    let guild_message = format!(
        "{} took moderation action `{custom_id}` against user {} using the broadcast embed buttons.",
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

async fn handle_unknown_ban(
    cache_http: impl CacheHttp,
    interaction_guild_id: GuildId,
    log_channel: &GuildChannel,
    target_user: &User,
) {
    let guild_message = format!(
        "Failed to unban user {}. Their ban was not found which most likely means they were not banned in the first place.",
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
            "Failed to inform guild {display_guild} that the unban using the broadcast embed buttons failed because of an unknown ban.",
        );
        Logger::get().error(&cache_http, e, log_msg).await;
    }
}
