use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::{ChannelType, GuildChannel, Role};

use crate::database::controllers::serverconfig_model_controller::{
    ActionLevel, ServerConfigComplete, ServerConfigModelController, UpdateServerConfig,
};
use crate::util::logger::Logger;
use crate::util::random_utils;
use crate::AppContext;
use crate::{assert_user_server, oops};

/// Subcommands for server configs.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands("display", "update", "enable_honeypot", "disable_honeypot"),
    subcommand_required
)]
pub async fn config(_: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

/// Display your own serverconfig.
#[poise::command(slash_command, guild_only = true)]
async fn display(ctx: AppContext<'_>) -> anyhow::Result<()> {
    ctx.defer().await?;
    assert_user_server!(ctx);

    // SAFETY: assert_user_server!() returns if guild_id is None
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
    #[description = "The level of action to take for users reported through honeypots."]
    honeypot_action_level: Option<ActionLevel>,
    #[description = "Role IDs to ignore when taking action. Separate multiple with a comma (,)."]
    ignored_roles: Option<String>,
) -> anyhow::Result<()> {
    ctx.defer().await?;
    assert_user_server!(ctx);

    // SAFETY: assert_user_server!() returns if guild_id is None
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
        honeypot_action_level,
        ignored_roles,
    };

    let updated =
        ServerConfigModelController::update(&ctx.data().db_pool, guild_id, update_values).await?;

    let embed = ServerConfigComplete::try_from_server_config(updated, &ctx.data().db_pool, &ctx)
        .await?
        .to_embed(ctx.author());

    let reply = CreateReply::default()
        .embed(embed)
        .content("Successfully updated your server config.");

    ctx.send(reply).await?;
    Ok(())
}

/// Use this command in the channel you want the honeypot to be.
#[poise::command(slash_command, guild_only = true)]
async fn enable_honeypot(ctx: AppContext<'_>) -> anyhow::Result<()> {
    ctx.defer_ephemeral().await?;
    assert_user_server!(ctx);

    let Some(channel) = ctx.guild_channel().await else {
        ctx.say("You somehow managed to use this command outside of a channel!")
            .await?;
        return Ok(());
    };

    if let Err(e) = ServerConfigModelController::add_honeypot_channel(
        &ctx.data().db_pool,
        channel.id,
        channel.guild_id,
        &ctx.data().honeypot_channels,
    )
    .await
    {
        let log_msg = format!("Failed to add channel {}", channel.id);
        Logger::get().error(ctx, e, log_msg).await;

        ctx.say("Failed to add honeypot channel to the database")
            .await?;
        return Ok(());
    }

    let message = format!(
        "Successfully added channel {} (`{}`) to your config.",
        channel.name, channel.id
    );

    ctx.say(message).await?;
    Ok(())
}

/// Disable the honeypot feature for your servers.
#[poise::command(slash_command, guild_only = true)]
async fn disable_honeypot(ctx: AppContext<'_>) -> anyhow::Result<()> {
    ctx.defer().await?;
    assert_user_server!(ctx);

    if let Err(e) = ServerConfigModelController::remove_honeypot_channel(
        &ctx.data().db_pool,
        // SAFETY: assert_user_server!() returns if guild_id is None
        ctx.guild_id().unwrap(),
        &ctx.data().honeypot_channels,
    )
    .await
    {
        let log_msg = "Failed to remove honeypot channel from the `server_configs` table";
        Logger::get().error(ctx, e, log_msg).await;

        ctx.say("Failed to add honeypot channel to the database")
            .await?;
        return Ok(());
    }

    ctx.say("Successfully removed honeypot channel from your config.")
        .await?;
    Ok(())
}
