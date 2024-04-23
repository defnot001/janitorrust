use anyhow::Context;
use poise::CreateReply;
use serde::{Deserialize, Serialize};
use serenity::all::{GuildId, User as SerenityUser, UserId};
use sqlx::PgPool;

use crate::{
    assert_admin, assert_admin_server,
    database::{
        serverconfig_model_controller::ServerConfigModelController,
        user_model_controller::{UserModelController, UserType},
    },
    oops,
    util::{
        builders::create_default_embed,
        format::{display, display_time, fdisplay},
        logger::Logger,
        random_utils::{get_guilds, get_users, parse_guild_ids},
    },
    Context as AppContext,
};

/// Subcommands for users.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands("list", "info", "add", "update", "remove"),
    subcommand_required
)]
pub async fn user(_: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

/// List users from a specific server.
#[poise::command(slash_command, guild_only = true)]
async fn list(
    ctx: AppContext<'_>,
    #[description = "The server ID you want to list the users for."] server_id: GuildId,
) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);
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
        .map(fdisplay)
        .collect::<Vec<String>>()
        .join("\n");

    let embed = create_default_embed(ctx.author())
        .title(format!("Whitelisted Users for {}", fdisplay(&guild)))
        .description(display_users);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Get information about a user.
#[poise::command(slash_command, guild_only = true)]
async fn info(
    ctx: AppContext<'_>,
    #[description = "The user you want info about."] user: SerenityUser,
) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);
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

    let guilds = match get_guilds(&db_user.servers, &ctx).await {
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
        .map(fdisplay)
        .collect::<Vec<String>>()
        .join("\n");

    let embed = create_default_embed(ctx.author())
        .title(format!("User Info for {}", fdisplay(&user)))
        .field("Server", display_guilds, false)
        .field("Created At", display_time(db_user.created_at), false);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Add a user to the databse.
#[poise::command(slash_command, guild_only = true)]
async fn add(
    ctx: AppContext<'_>,
    #[description = "The user to add to the whitelist."] user: SerenityUser,
    #[description = "Server(s) for bot usage, separated by commas."] servers: String,
    #[description = "Wether the user can only receive reports or also create them."]
    user_type: UserType,
) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);
    let logger = Logger::get();
    ctx.defer().await?;

    let guild_ids = match parse_guild_ids(&servers) {
        Ok(ids) => ids,
        Err(e) => {
            let user_msg = "Failed to parse your provided guild IDs!";
            oops!(ctx, user_msg);
        }
    };

    let guilds = match get_guilds(&guild_ids, &ctx).await {
        Ok(guilds) => guilds,
        Err(e) => {
            let log_msg = format!(
                "Failed to get one or more guilds for {} from the discord api",
                user
            );
            logger.error(&ctx, e, log_msg).await;

            let user_msg = format!("Could not get one or more guild(s) for {}!", user);
            oops!(ctx, user_msg);
        }
    };

    let added_user = match UserModelController::create(
        &ctx.data().db_pool,
        user.id,
        user_type,
        &guild_ids,
    )
    .await
    {
        Ok(user) => user,
        Err(e) => {
            if e.to_string().starts_with("Unique") {
                let msg = format!("User {} is already in the database!", fdisplay(&user));
                oops!(ctx, msg);
            } else {
                let log_msg = format!("Failed to add user {} to the database", display(&user));
                logger.error(&ctx, e, log_msg).await;

                let user_msg = format!("Failed to add user {} to the database!", fdisplay(&user));
                oops!(ctx, user_msg);
            }
        }
    };

    if let Err(e) = handle_server_config_updates(&ctx.data().db_pool, &[], &guild_ids).await {
        let log_msg = "Failed handle potential server config updates";
        logger.error(&ctx, e, log_msg).await;
    }

    ctx.send(
        CreateReply::default()
            .embed(added_user.to_embed(ctx.author(), &user, &guilds))
            .content("User added to the database!"),
    )
    .await?;

    Ok(())
}

/// Update a user in the database
#[poise::command(slash_command, guild_only = true)]
async fn update(ctx: AppContext<'_>) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);

    let logger = Logger::get();
    ctx.defer().await?;

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
async fn remove(ctx: AppContext<'_>) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);
    let logger = Logger::get();
    ctx.defer().await?;
    Ok(())
}

async fn handle_server_config_updates(
    db_pool: &PgPool,
    old_ids: &[GuildId],
    new_ids: &[GuildId],
) -> anyhow::Result<()> {
    let add_res = futures::future::try_join_all(
        new_ids
            .iter()
            .filter(|&id| !old_ids.contains(id))
            .map(|&g| ServerConfigModelController::create_default_if_not_exists(db_pool, g)),
    );

    let remove_res = futures::future::try_join_all(
        old_ids
            .iter()
            .filter(|&id| !new_ids.contains(id))
            .map(|&g| ServerConfigModelController::delete_if_needed(db_pool, g)),
    );

    tokio::try_join!(add_res, remove_res).context("Failed to handle server config updates")?;

    Ok(())
}
