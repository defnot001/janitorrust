use anyhow::Context;
use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::{GuildId, User as SerenityUser, UserId};
use sqlx::PgPool;

use crate::database::controllers::serverconfig_model_controller::ServerConfigModelController;
use crate::database::controllers::user_model_controller::CreateJanitorUser;
use crate::database::controllers::user_model_controller::{UserModelController, UserType};
use crate::util::{embeds, format, random_utils};
use crate::{assert_admin, assert_admin_server};
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
    ctx.defer().await?;

    let guild = server_id.to_partial_guild(&ctx).await?;

    let user_ids = UserModelController::get_by_guild(&ctx.data().db_pool, guild.id)
        .await?
        .into_iter()
        .map(|u| u.user_id)
        .collect::<Vec<UserId>>();

    let display_users = random_utils::get_users(user_ids, &ctx)
        .await?
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
    ctx.defer().await?;

    let Some(db_user) = UserModelController::get(&ctx.data().db_pool, user.id).await? else {
        let reply = format!(
            "User {} does not exist in the database!",
            format::fdisplay(&user)
        );
        ctx.say(reply).await?;
        return Ok(());
    };

    let display_guilds = random_utils::get_guilds(&db_user.guild_ids, &ctx)
        .await?
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
    ctx.defer().await?;

    let guild_ids = random_utils::parse_guild_ids(&servers)?;
    let guilds = random_utils::get_guilds(&guild_ids, &ctx).await?;

    let create_user = CreateJanitorUser {
        guild_ids: &guild_ids,
        user_id: user.id,
        user_type,
    };

    let added_user = UserModelController::create(&ctx.data().db_pool, create_user).await?;

    if let Err(e) = handle_server_config_updates(&ctx.data().db_pool, &[], &guild_ids).await {
        let log_msg = "Failed handle potential server config updates";
        Logger::get().error(ctx, e, log_msg).await;
    }

    let reply = CreateReply::default()
        .embed(added_user.to_embed(ctx.author(), &user, &guilds))
        .content("User added to the database!");

    ctx.send(reply).await?;
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
    ctx.defer().await?;

    let new_guild_ids = if let Some(servers) = servers {
        let parsed = random_utils::parse_guild_ids(&servers)?;
        Some(parsed)
    } else {
        None
    };

    let old_user = match UserModelController::get(&ctx.data().db_pool, user.id).await? {
        Some(user) => user,
        None => {
            let reply = format!(
                "User {} does not exist in the database! Consider adding them by using `/user add`.",
                format::fdisplay(&user)
            );
            ctx.say(reply).await?;
            return Ok(());
        }
    };

    let updated_user_type = user_type.unwrap_or(old_user.user_type);
    let updated_ids = new_guild_ids.unwrap_or(old_user.guild_ids.clone());
    let updated_guilds = random_utils::get_guilds(&updated_ids, &ctx).await?;

    let create_user = CreateJanitorUser {
        user_id: user.id,
        guild_ids: &updated_ids,
        user_type: updated_user_type,
    };

    let updated_user = UserModelController::update(&ctx.data().db_pool, create_user).await?;

    if let Err(e) =
        handle_server_config_updates(&ctx.data().db_pool, &old_user.guild_ids, &updated_ids).await
    {
        let log_msg = "Failed handle potential server config updates";
        Logger::get().error(ctx, e, log_msg).await;
    }

    let reply = CreateReply::default()
        .embed(updated_user.to_embed(ctx.author(), &user, &updated_guilds))
        .content("User updated in the database!");

    ctx.send(reply).await?;
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
async fn remove(
    ctx: AppContext<'_>,
    #[description = "The user to deleted from the whitelist."] user: SerenityUser,
) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);
    ctx.defer().await?;

    let deleted_user = UserModelController::delete(&ctx.data().db_pool, user.id).await?;

    if let Err(e) =
        handle_server_config_updates(&ctx.data().db_pool, &deleted_user.guild_ids, &[]).await
    {
        let log_msg = "Failed handle potential server config updates";
        Logger::get().error(ctx, e, log_msg).await;
    }

    let reply = format!(
        "Successfully removed user {} from the database.",
        format::fdisplay(&user)
    );

    ctx.say(reply).await?;
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
