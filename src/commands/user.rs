use anyhow::Context;
use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::{GuildId, User as SerenityUser, UserId};
use sqlx::PgPool;

use crate::database::serverconfig_model_controller::ServerConfigModelController;
use crate::database::user_model_controller::{UserModelController, UserType};
use crate::util::{embeds, format, random_utils};
use crate::{assert_admin, assert_admin_server, oops};
use crate::{Context as AppContext, Logger};

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
            logger.error(ctx, e, &msg).await;
            oops!(ctx, msg);
        }
    };

    let user_ids = match UserModelController::get_by_guild(&ctx.data().db_pool, &guild.id).await {
        Ok(users) => users.into_iter().map(|u| u.id).collect::<Vec<UserId>>(),
        Err(e) => {
            let log_msg = format!(
                "Failed to get users for {} from the database",
                format::display(&guild)
            );
            logger.error(ctx, e, log_msg).await;

            let user_msg = format!(
                "Failed to get users for {} from the database!",
                format::fdisplay(&guild)
            );
            oops!(ctx, user_msg);
        }
    };

    let users = match random_utils::get_users(user_ids, &ctx).await {
        Ok(users) => users,
        Err(e) => {
            let log_msg = format!(
                "Failed to get user objects for {} from the discord API",
                format::display(&guild)
            );
            logger.error(ctx, e, log_msg).await;

            let user_msg = format!(
                "Failed to get users for {} from the Discord API!",
                format::fdisplay(&guild)
            );
            oops!(ctx, user_msg);
        }
    };

    let display_users = users
        .iter()
        .map(format::display)
        .collect::<Vec<String>>()
        .join("\n");

    let embed = embeds::CreateJanitorEmbed::new(ctx.author())
        .into_embed()
        .title(format!(
            "Whitelisted Users for {}",
            format::fdisplay(&guild)
        ))
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
            let log_msg = format!(
                "Failed to get user {} from the databse",
                format::display(&user)
            );
            logger.error(ctx, e, log_msg).await;

            let user_msg = format!(
                "Failed to get user {} from the databse!",
                format::fdisplay(&user)
            );
            oops!(ctx, user_msg);
        }
    };

    let db_user = match db_user {
        Some(user) => user,
        None => {
            let user_msg = format!(
                "User {} does not exist in the database!",
                format::fdisplay(&user)
            );
            oops!(ctx, user_msg);
        }
    };

    let guilds = match random_utils::get_guilds(&db_user.servers, &ctx).await {
        Ok(guilds) => guilds,
        Err(e) => {
            let log_msg = format!(
                "Failed to fetch one or more guilds for {} from the api",
                format::display(&user)
            );
            logger.error(ctx, e, log_msg).await;

            let user_msg = format!(
                "Failed to fetch one or more guilds for user {} from the Discord API!",
                format::fdisplay(&user)
            );
            oops!(ctx, user_msg);
        }
    };

    let display_guilds = guilds
        .iter()
        .map(format::fdisplay)
        .collect::<Vec<String>>()
        .join("\n");

    let embed = embeds::CreateJanitorEmbed::new(ctx.author())
        .into_embed()
        .title(format!("User Info for {}", format::fdisplay(&user)))
        .field("Server", display_guilds, false)
        .field(
            "Created At",
            format::display_time(db_user.created_at),
            false,
        );

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

    let guild_ids = match random_utils::parse_guild_ids(&servers) {
        Ok(ids) => ids,
        Err(_) => {
            let user_msg = "Failed to parse your provided guild IDs!";
            oops!(ctx, user_msg);
        }
    };

    let guilds = match random_utils::get_guilds(&guild_ids, &ctx).await {
        Ok(guilds) => guilds,
        Err(e) => {
            let log_msg = format!(
                "Failed to get one or more guilds for {} from the discord api",
                user
            );
            logger.error(ctx, e, log_msg).await;

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
                let msg = format!(
                    "User {} is already in the database!",
                    format::fdisplay(&user)
                );
                oops!(ctx, msg);
            } else {
                let log_msg = format!(
                    "Failed to add user {} to the database",
                    format::display(&user)
                );
                logger.error(ctx, e, log_msg).await;

                let user_msg = format!(
                    "Failed to add user {} to the database!",
                    format::fdisplay(&user)
                );
                oops!(ctx, user_msg);
            }
        }
    };

    if let Err(e) = handle_server_config_updates(&ctx.data().db_pool, &[], &guild_ids).await {
        let log_msg = "Failed handle potential server config updates";
        logger.error(ctx, e, log_msg).await;
    }

    ctx.send(
        CreateReply::default()
            .embed(added_user.to_embed(ctx.author(), &user, &guilds))
            .content("User added to the database!"),
    )
    .await?;

    Ok(())
}

/// Update a user in the database.
#[poise::command(slash_command, guild_only = true)]
async fn update(
    ctx: AppContext<'_>,
    #[description = "The user to add update on the whitelist."] user: SerenityUser,
    #[description = "Server(s) for bot usage, separated by commas."] servers: Option<String>,
    #[description = "Wether the user can only receive reports or also create them."]
    user_type: Option<UserType>,
) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);
    let logger = Logger::get();
    ctx.defer().await?;

    let new_guild_ids = if let Some(servers) = servers {
        match random_utils::parse_guild_ids(&servers) {
            Ok(guild_ids) => Some(guild_ids),
            Err(_) => {
                let user_msg = "Failed to parse your provided guild IDs!";
                oops!(ctx, user_msg);
            }
        }
    } else {
        None
    };

    let old_user = match UserModelController::get(&ctx.data().db_pool, user.id).await {
        Ok(user) => user,
        Err(e) => {
            let log_msg = format!(
                "Failed to get user {} to update in the database",
                format::display(&user)
            );
            logger.error(ctx, e, log_msg).await;

            let user_msg = format!(
                "Failed to get user {} to update from the database!",
                format::fdisplay(&user)
            );
            oops!(ctx, user_msg);
        }
    };

    let old_user = match old_user {
        Some(user) => user,
        None => {
            let user_msg = format!(
                "User {} does not exist in the database! Consider adding them by using `/user add`.",
                format::fdisplay(&user)
            );
            oops!(ctx, user_msg);
        }
    };

    let user_type = user_type.unwrap_or(old_user.user_type);
    let updated_ids = new_guild_ids.unwrap_or(old_user.servers.clone());

    let updated_guilds = match random_utils::get_guilds(&updated_ids, &ctx).await {
        Ok(guilds) => guilds,
        Err(e) => {
            let log_msg = format!(
                "Failed to get one or more guilds for {} from the discord api",
                user
            );
            logger.error(ctx, e, log_msg).await;

            let user_msg = format!("Could not get one or more guild(s) for {}!", user);
            oops!(ctx, user_msg);
        }
    };

    let updated_user =
        match UserModelController::update(&ctx.data().db_pool, user.id, user_type, &updated_ids)
            .await
        {
            Ok(updated) => updated,
            Err(e) => {
                let log_msg = format!(
                    "Failed to updated user {} in the database",
                    format::display(&user)
                );
                logger.error(ctx, e, log_msg).await;

                let user_msg = format!(
                    "Failed to updated user {} in the database",
                    format::fdisplay(&user)
                );
                oops!(ctx, user_msg);
            }
        };

    if let Err(e) =
        handle_server_config_updates(&ctx.data().db_pool, &old_user.servers, &updated_ids).await
    {
        let log_msg = "Failed handle potential server config updates";
        logger.error(ctx, e, log_msg).await;
    }

    ctx.send(
        CreateReply::default()
            .embed(updated_user.to_embed(ctx.author(), &user, &updated_guilds))
            .content("User updated in the database!"),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
async fn remove(
    ctx: AppContext<'_>,
    #[description = "The user to deleted from the whitelist."] user: SerenityUser,
) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);
    let logger = Logger::get();
    ctx.defer().await?;

    let deleted_user = match UserModelController::delete(&ctx.data().db_pool, user.id).await {
        Ok(user) => user,
        Err(e) => {
            let log_msg = format!(
                "Failed to delete user {} from the database",
                format::display(&user)
            );
            logger.error(ctx, e, log_msg).await;

            let user_msg = format!(
                "Failed to delete user {} from the database",
                format::fdisplay(&user)
            );
            oops!(ctx, user_msg);
        }
    };

    if let Err(e) =
        handle_server_config_updates(&ctx.data().db_pool, &deleted_user.servers, &[]).await
    {
        let log_msg = "Failed handle potential server config updates";
        logger.error(ctx, e, log_msg).await;
    }

    ctx.say(format!(
        "Successfully removed user {} from the database.",
        format::fdisplay(&user)
    ))
    .await?;

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
