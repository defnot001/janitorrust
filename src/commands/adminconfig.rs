use poise::CreateReply;
use serenity::all::CacheHttp;

use crate::database::controllers::badactor_model_controller::BadActorModelController;
use crate::database::controllers::serverconfig_model_controller::{
    ServerConfigComplete, ServerConfigModelController,
};
use crate::util::embeds::CreateJanitorEmbed;
use crate::util::format::display_guild_ids;
use crate::util::parsing::parse_guild_ids;
use crate::util::screenshot::FileManager;
use crate::AppContext;
use crate::{assert_admin, assert_admin_server};

/// Subcommands for admins to inspect the bot's server configs.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands(
        "display_configs",
        "delete_bad_actor",
        "display_config_guilds",
        "display_guilds"
    ),
    subcommand_required
)]
pub async fn adminconfig(_: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

/// Display the configs for up to 5 servers at a time.
#[poise::command(slash_command)]
async fn display_configs(
    ctx: AppContext<'_>,
    #[description = "The ID(s) of the server(s) to display the config for. Separate multiple IDs with a comma (,). Max 5."]
    guild_id: String,
) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);
    ctx.defer().await?;

    let guild_ids = parse_guild_ids(&guild_id)?;

    let async_iter =
        ServerConfigModelController::get_multiple_by_guild_id(&ctx.data().db_pool, &guild_ids)
            .await?
            .into_iter()
            .map(|c| async {
                let config =
                    ServerConfigComplete::try_from_server_config(c, &ctx.data().db_pool, &ctx)
                        .await;

                match config {
                    Ok(c) => Ok(c.to_embed(ctx.author())),
                    Err(e) => anyhow::bail!(e),
                }
            });

    let embeds = futures::future::try_join_all(async_iter).await?;

    let reply = CreateReply {
        embeds,
        ..Default::default()
    };

    ctx.send(reply).await?;
    Ok(())
}

/// Display all guilds that currently have a config for Janitor.
#[poise::command(slash_command)]
async fn display_config_guilds(ctx: AppContext<'_>) -> anyhow::Result<()> {
    ctx.defer().await?;
    assert_admin!(ctx);
    assert_admin_server!(ctx);

    let guild_ids = ServerConfigModelController::get_all_guild_ids(&ctx.data().db_pool).await?;

    let embed = CreateJanitorEmbed::new(ctx.author())
        .into_embed()
        .title("Servers with Janitor config")
        .description(display_guild_ids(&ctx, &guild_ids, true).await?);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Display all guilds that the bot is currently in.
#[poise::command(slash_command)]
async fn display_guilds(ctx: AppContext<'_>) -> anyhow::Result<()> {
    ctx.defer().await?;
    assert_admin!(ctx);
    assert_admin_server!(ctx);

    let Some(cache) = ctx.serenity_context().cache() else {
        ctx.say("Failed to get the bot's cache.").await?;
        return Ok(());
    };

    let embed = CreateJanitorEmbed::new(ctx.author())
        .into_embed()
        .title("Servers Janitor is in")
        .description(display_guild_ids(&ctx, &cache.guilds(), true).await?);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

/// Delete a bad actor from the database.
#[poise::command(slash_command)]
async fn delete_bad_actor(
    ctx: AppContext<'_>,
    #[description = "The entry id that you want to delete."] entry: i32,
) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);
    ctx.defer().await?;

    let deleted = BadActorModelController::delete(&ctx.data().db_pool, entry).await?;

    if let Some(file_name) = deleted.screenshot_proof.as_ref() {
        FileManager::delete(file_name).await?;
    }

    let reply = format!("Successfully deleted bad actor entry with id {entry} from the database.");

    ctx.say(reply).await?;
    Ok(())
}
