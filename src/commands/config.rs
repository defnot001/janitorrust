use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::{ChannelType, GuildChannel, Role};

use crate::database::controllers::serverconfig_model_controller::{
    ActionLevel, ServerConfigComplete, ServerConfigModelController, UpdateServerConfig,
};
use crate::util::random_utils;
use crate::AppContext;
use crate::{assert_user_server, oops};

/// Subcommands for server configs.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands("display", "update"),
    subcommand_required
)]
pub async fn config(_: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

/// Display your own serverconfig.
#[poise::command(slash_command, guild_only = true)]
async fn display(ctx: AppContext<'_>) -> anyhow::Result<()> {
    assert_user_server!(ctx);
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();

    let Some(config) =
        ServerConfigModelController::get_by_guild_id(&ctx.data().db_pool, guild_id).await?
    else {
        let user_msg = "Your server doesn't have a config in the database!";
        oops!(ctx, user_msg);
    };

    let embed = ServerConfigComplete::try_from_server_config(config, &ctx.data().db_pool, &ctx)
        .await?
        .to_embed(ctx.author());

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Update your own serverconfig.
#[poise::command(slash_command, guild_only = true)]
#[allow(clippy::too_many_arguments)]
async fn update(
    ctx: AppContext<'_>,
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
    #[description = "Role IDs to ignore when taking action. Separate multiple with a comma (,)."]
    ignored_roles: Option<String>,
) -> anyhow::Result<()> {
    assert_user_server!(ctx);
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();

    if let Some(c) = &log_channel {
        if c.kind != ChannelType::Text {
            ctx.say(format!("{} is not a text channel.", c.name))
                .await?;
            return Ok(());
        }
    }

    let ignored_roles = if let Some(r) = ignored_roles {
        Some(random_utils::parse_role_ids(&r)?)
    } else {
        None
    };

    let log_channel_id = log_channel.map(|c| c.id);
    let ping_role = ping_role.map(|r| r.id);

    let update_values = UpdateServerConfig {
        log_channel_id,
        ping_users,
        ping_role,
        spam_action_level,
        impersonation_action_level,
        bigotry_action_level,
        ignored_roles,
    };

    let updated = ServerConfigModelController::update(
        &ctx.data().db_pool,
        guild_id,
        &ctx.data().honeypot_channels,
        update_values,
    )
    .await?;

    let embed = ServerConfigComplete::try_from_server_config(updated, &ctx.data().db_pool, &ctx)
        .await?
        .to_embed(ctx.author());

    let reply = CreateReply::default()
        .embed(embed)
        .content("Successfully updated your server config.");

    ctx.send(reply).await?;
    Ok(())
}
