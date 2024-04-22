use poise::CreateReply;
use serde::{Deserialize, Serialize};
use serenity::all::{GuildId, User as SerenityUser, UserId};

use crate::{
    assert_admin, assert_admin_server,
    database::user_model_controller::UserModelController,
    oops,
    util::{
        builders::create_default_embed,
        format::{display, display_time, fdisplay},
        logger::Logger,
        random_utils::{get_guilds, get_users},
    },
    Context,
};

/// Subcommands for users.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands("list", "info", "add", "update", "remove"),
    subcommand_required
)]
pub async fn user(_: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
async fn list(
    ctx: Context<'_>,
    #[description = "The server ID you want to list the users for."] server_id: GuildId,
) -> anyhow::Result<()> {
    assert_admin_server!(ctx);
    assert_admin!(ctx);
    let logger = Logger::get();
    ctx.defer().await?;

    let guild = match server_id.to_partial_guild(&ctx).await {
        Ok(guild) => guild,
        Err(e) => {
            let msg = format!("Failed to get guild `{server_id}` from the API!");
            logger.error(&ctx, e, &msg).await;
            oops!(ctx, msg);
        }
    };

    let user_ids = match UserModelController::get_by_guild(&ctx.data().db_pool, &guild.id).await {
        Ok(users) => users.into_iter().map(|u| u.id).collect::<Vec<UserId>>(),
        Err(e) => {
            let log_msg = format!(
                "Failed to get users for {} from the database",
                display(&guild)
            );
            logger.error(&ctx, e, log_msg).await;

            let user_msg = format!(
                "Failed to get users for {} from the database!",
                fdisplay(&guild)
            );
            oops!(ctx, user_msg);
        }
    };

    let users = match get_users(user_ids, &ctx).await {
        Ok(users) => users,
        Err(e) => {
            let log_msg = format!(
                "Failed to get user objects for {} from the discord API",
                display(&guild)
            );
            logger.error(&ctx, e, log_msg).await;

            let user_msg = format!(
                "Failed to get users for {} from the Discord API!",
                fdisplay(&guild)
            );
            oops!(ctx, user_msg);
        }
    };

    let display_users = users
        .iter()
        .map(|u| fdisplay(u))
        .collect::<Vec<String>>()
        .join("\n");

    let embed = create_default_embed(ctx.author())
        .title(format!("Whitelisted Users for {}", fdisplay(&guild)))
        .description(display_users);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
async fn info(
    ctx: Context<'_>,
    #[description = "The user you want info about."] user: SerenityUser,
) -> anyhow::Result<()> {
    assert_admin_server!(ctx);
    assert_admin!(ctx);
    let logger = Logger::get();
    ctx.defer().await?;

    let db_user = match UserModelController::get(&ctx.data().db_pool, user.id).await {
        Ok(user) => user,
        Err(e) => {
            let log_msg = format!("Failed to get user {} from the databse", display(&user));
            logger.error(&ctx, e, log_msg).await;

            let user_msg = format!("Failed to get user {} from the databse!", fdisplay(&user));
            oops!(ctx, user_msg);
        }
    };

    let db_user = match db_user {
        Some(user) => user,
        None => {
            let user_msg = format!("User {} does not exist in the database!", fdisplay(&user));
            oops!(ctx, user_msg);
        }
    };

    let guilds = match get_guilds(db_user.servers, &ctx).await {
        Ok(guilds) => guilds,
        Err(e) => {
            let log_msg = format!(
                "Failed to fetch one or more guilds for {} from the api",
                display(&user)
            );
            logger.error(&ctx, e, log_msg).await;

            let user_msg = format!(
                "Failed to fetch one or more guilds for user {} from the Discord API!",
                fdisplay(&user)
            );
            oops!(ctx, user_msg);
        }
    };

    let display_guilds = guilds
        .iter()
        .map(|g| fdisplay(g))
        .collect::<Vec<String>>()
        .join("\n");

    let embed = create_default_embed(ctx.author())
        .title(format!("User Info for {}", fdisplay(&user)))
        .field("Server", display_guilds, false)
        .field("Created At", display_time(db_user.created_at), false);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
async fn add(ctx: Context<'_>) -> anyhow::Result<()> {
    assert_admin_server!(ctx);
    assert_admin!(ctx);
    let logger = Logger::get();
    ctx.defer().await?;
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
async fn update(ctx: Context<'_>) -> anyhow::Result<()> {
    assert_admin_server!(ctx);
    assert_admin!(ctx);
    let logger = Logger::get();
    ctx.defer().await?;
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
async fn remove(ctx: Context<'_>) -> anyhow::Result<()> {
    assert_admin_server!(ctx);
    assert_admin!(ctx);
    let logger = Logger::get();
    ctx.defer().await?;
    Ok(())
}
