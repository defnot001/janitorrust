use poise::CreateReply;

use crate::database::badactor_model_controller::BadActorModelController;
use crate::database::serverconfig_model_controller::{
    ServerConfigComplete, ServerConfigModelController,
};
use crate::util::logger::Logger;
use crate::util::random_utils::parse_guild_ids;
use crate::util::screenshot::FileManager;
use crate::Context as AppContext;
use crate::{assert_admin, assert_admin_server, oops};

/// Subcommands for admins to inspect the bot's server configs.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands("display_configs", "delete_bad_actor"),
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

    let logger = Logger::get();

    ctx.defer().await?;

    let guild_ids = match parse_guild_ids(&guild_id) {
        Ok(ids) => {
            if ids.is_empty() || ids.len() > 5 {
                let user_msg = format!("Expected between 1 and 5 guilds, got {}", ids.len());
                oops!(ctx, user_msg);
            }

            ids
        }
        Err(_) => {
            let user_msg = "One or more of the guilds ids you provided are invalid!";
            oops!(ctx, user_msg);
        }
    };

    let target_amount = guild_ids.len();

    let configs = match ServerConfigModelController::get_multiple_by_guild_id(
        &ctx.data().db_pool,
        &guild_ids,
    )
    .await
    {
        Ok(configs) => configs,
        Err(e) => {
            let log_msg = format!(
                "Failed to query database for serverconfigs for guilds {:?}",
                &guild_ids
            );
            logger.error(ctx, e, log_msg).await;

            let user_msg = "Failed to get the config for one or more servers from the database";
            oops!(ctx, user_msg);
        }
    };

    let mut embeds = Vec::with_capacity(target_amount);

    for config in configs {
        let guild_id = config.server_id;

        match ServerConfigComplete::try_from_server_config(config, ctx).await {
            Ok(c) => {
                embeds.push(c.to_embed(ctx.author()));
            }
            Err(e) => {
                let log_msg =
                    format!("Failed to upgrade server config for {guild_id} to full config");
                logger.error(ctx, e, log_msg).await;

                let user_msg =
                    format!("There was an error getting the config for server {guild_id}.");
                oops!(ctx, user_msg);
            }
        };
    }

    ctx.send(CreateReply {
        embeds,
        ..Default::default()
    })
    .await?;

    Ok(())
}

/// Delete a bad actor from the database.
#[poise::command(slash_command)]
async fn delete_bad_actor(
    ctx: AppContext<'_>,
    #[description = "The entry id that you want to delete."] entry: u64,
) -> anyhow::Result<()> {
    assert_admin!(ctx);
    assert_admin_server!(ctx);

    let logger = Logger::get();

    ctx.defer().await?;

    let deleted = match BadActorModelController::delete(&ctx.data().db_pool, entry).await {
        Ok(deleted) => deleted,
        Err(e) => {
            let msg = format!("Failed to delete entry with id {entry} from the database");
            logger.error(ctx, e, &msg).await;
            oops!(ctx, msg);
        }
    };

    tracing::info!("Deleted bad actor entry with id {entry} from the database.");

    if let Some(file_name) = deleted.screenshot_proof.as_ref() {
        if let Err(e) = FileManager::delete(file_name).await {
            let log_msg = format!("Failed to delete screenshot {file_name} from the file system");
            logger.error(ctx, e, log_msg).await;

            let user_msg = format!("Bad actor with id {entry} was successfully deleted from the database but deleting the screenshot failed. Please do so manually");
            oops!(ctx, user_msg);
        }

        tracing::info!("Deleted screenshot {file_name} from the file system.");
    }

    ctx.say(format!("Successfully deleted bad actor entry with id {entry} from the database. If they had a screenshot, it was also deleted.")).await?;
    Ok(())
}
