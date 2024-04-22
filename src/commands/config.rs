use std::num::NonZeroU64;

use poise::CreateReply;
use serde::{Deserialize, Serialize};
use serenity::all::{ChannelType, GuildChannel, Role, RoleId};

use crate::{
    assert_user_server,
    database::serverconfig_model_controller::{
        ActionLevel, ServerConfigComplete, ServerConfigModelController, UpdateServerConfig,
    },
    oops,
    util::logger::Logger,
    Context,
};

/// Subcommands for server configs.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands("display", "update"),
    subcommand_required
)]
pub async fn config(_: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

/// Display your own serverconfig.
#[poise::command(slash_command, guild_only = true)]
async fn display(ctx: Context<'_>) -> anyhow::Result<()> {
    assert_user_server!(ctx);
    let guild_id = ctx.guild_id().unwrap();

    let logger = Logger::get();

    ctx.defer().await?;

    let config =
        match ServerConfigModelController::get_by_guild_id(&ctx.data().db_pool, guild_id).await {
            Ok(config) => match config {
                Some(config) => config,
                None => {
                    let user_msg = "Cannot find the config for your server in the database!";
                    oops!(ctx, user_msg);
                }
            },
            Err(e) => {
                let log_msg = format!("Failed to query db for server config for {guild_id}");
                logger.error(&ctx, e, log_msg);

                let user_msg = "Failed to get server config from database!";
                oops!(ctx, user_msg);
            }
        };

    let embed =
        match ServerConfigComplete::try_from_server_config(config, &ctx.data().db_pool, &ctx).await
        {
            Ok(config) => config.to_embed(ctx.author()),
            Err(e) => {
                let log_msg =
                    format!("Failed to upgrade server config for {guild_id} to full config");
                logger.error(&ctx, e, log_msg).await;

                let user_msg = "Failed to get server config from database!";
                oops!(ctx, user_msg);
            }
        };

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Update your own serverconfig.
#[poise::command(slash_command, guild_only = true)]
#[allow(clippy::too_many_arguments)]
async fn update(
    ctx: Context<'_>,
    #[description = "The channel to log actions to."] log_channel: Option<GuildChannel>,
    #[description = "Ping users when action is taken."] ping_users: Option<bool>,
    #[description = "The role to ping when action is taken. Set this to the bot itself to remove."]
    ping_role: Option<Role>,
    #[description = "The level of action to take for spamming users with hacked accounts."]
    spam_action_level: Option<ActionLevel>,
    #[description = "The level of action to take for users impersonating others."]
    impersonation_action_level: Option<ActionLevel>,
    #[description = "The level of action to take for users with bigot behaviour."]
    bigotry_action_level: Option<ActionLevel>,
    #[description = "Whether or not to timeout users with a specific role."]
    timeout_users_with_role: Option<bool>,
    #[description = "Role IDs to ignore when taking action. Separate multiple with a comma (,)."]
    ignored_roles: Option<String>,
) -> anyhow::Result<()> {
    assert_user_server!(ctx);
    let guild_id = ctx.guild_id().unwrap();

    let logger = Logger::get();

    ctx.defer().await?;

    if let Some(c) = &log_channel {
        if c.kind != ChannelType::Text {
            let msg = format!("{} is not a text channel.", c.name);
            oops!(ctx, msg);
        }
    }

    let ignored_roles = if let Some(r) = ignored_roles {
        Some(parse_role_ids(&r)?)
    } else {
        None
    };

    let update = UpdateServerConfig {
        log_channel: log_channel.map(|c| c.id),
        ping_users,
        ping_role: ping_role.map(|r| r.id),
        spam_action_level,
        impersonation_action_level,
        bigotry_action_level,
        timeout_users_with_role,
        ignored_roles,
    };

    let updated =
        match ServerConfigModelController::update(&ctx.data().db_pool, guild_id, update).await {
            Ok(updated) => updated,
            Err(e) => {
                let log_msg = format!("Failed to update server config for {guild_id}");
                logger.error(&ctx, e, log_msg).await;

                let user_msg = "Failed to update server config for your server!";
                oops!(ctx, user_msg);
            }
        };

    let embed = match ServerConfigComplete::try_from_server_config(
        updated,
        &ctx.data().db_pool,
        &ctx,
    )
    .await
    {
        Ok(config) => config.to_embed(ctx.author()),
        Err(e) => {
            let log_msg = format!("Failed to upgrade server config for {guild_id} to full config");
            logger.error(&ctx, e, log_msg).await;

            let user_msg = "Failed to update server config for your server in the datbase!";
            oops!(ctx, user_msg);
        }
    };

    ctx.send(
        CreateReply::default()
            .embed(embed)
            .content("Successfully updated your server config."),
    )
    .await?;

    Ok(())
}

fn parse_role_ids(str: &str) -> anyhow::Result<Vec<RoleId>> {
    str.split(',')
        .map(|id| match id.parse::<u64>() {
            Ok(id) => {
                if let Some(non_zero) = NonZeroU64::new(id) {
                    Ok(RoleId::from(non_zero))
                } else {
                    anyhow::bail!("0 is not a valid role id")
                }
            }
            Err(e) => anyhow::bail!(e),
        })
        .collect()
}
