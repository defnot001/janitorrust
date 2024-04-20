use std::num::NonZeroU64;

use poise::CreateReply;
use serenity::all::GuildId;

use crate::{
    assert_admin, assert_admin_server,
    database::{
        badactor_model_controller::BadActorModelController,
        serverconfig_model_controller::{ServerConfigComplete, ServerConfigModelController},
    },
    util::{
        error::{respond_error, respond_mistake},
        screenshot::FileManager,
    },
    Context,
};

/// Subcommands for admins to inspect the bot's server configs.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands("display_configs", "delete_bad_actor"),
    subcommand_required
)]
pub async fn adminconfig(_: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

/// Display the configs for up to 5 servers at a time.
#[poise::command(slash_command)]
async fn display_configs(
    ctx: Context<'_>,
    #[description = "The ID(s) of the server(s) to display the config for. Separate multiple IDs with a comma (,). Max 5."]
    guild_id: String,
) -> anyhow::Result<()> {
    assert_admin_server!(&ctx);
    assert_admin!(&ctx);
    ctx.defer().await?;

    let guild_ids = match parse_guild_ids(&guild_id) {
        Ok(ids) => {
            if ids.is_empty() || ids.len() > 5 {
                let msg = format!("Expected between 1 and 5 guilds, got {}", ids.len());
                return respond_mistake(msg, &ctx).await;
            }

            ids
        }
        Err(e) => {
            let msg = "One or more of the guilds ids you provided are invalid";
            return respond_mistake(msg, &ctx).await;
        }
    };

    let target_amount = guild_ids.len();

    let configs =
        match ServerConfigModelController::get_multiple_by_guild_id(&ctx.data().db_pool, guild_ids)
            .await
        {
            Ok(configs) => configs,
            Err(e) => {
                let msg = "Failed to get the config for one or more servers from the database";
                return respond_error(msg, e, &ctx).await;
            }
        };

    let mut embeds = Vec::with_capacity(target_amount);

    for config in configs {
        let full_config =
            ServerConfigComplete::from_server_config(config, &ctx.data().db_pool, &ctx).await?;

        embeds.push(full_config.to_embed(ctx.author()))
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
    ctx: Context<'_>,
    #[description = "The entry id that you want to delete."] entry: u64,
) -> anyhow::Result<()> {
    assert_admin_server!(&ctx);
    assert_admin!(&ctx);
    ctx.defer().await?;

    let deleted = match BadActorModelController::delete(&ctx.data().db_pool, entry).await {
        Ok(deleted) => deleted,
        Err(e) => {
            let msg = format!("Failed to delete entry with id {entry} from the database");
            return respond_error(msg, e, &ctx).await;
        }
    };

    tracing::info!("Deleted bad actor entry with id {entry} from the database.");

    if let Some(file_name) = deleted.screenshot_proof.as_ref() {
        if let Err(e) = FileManager::delete(file_name).await {
            let msg = format!("Bad actor with id {entry} was successfully deleted from the database but deleting the screenshot failed. Please do so manually");
            return respond_error(msg, e, &ctx).await;
        }

        tracing::info!("Deleted screenshot {file_name} from the file system.");
    }

    ctx.say(format!("Successfully deleted bad actor entry with id {entry} from the database. If they had a screenshot, it was also deleted.")).await?;
    Ok(())
}

fn parse_guild_ids(str: &str) -> anyhow::Result<Vec<GuildId>> {
    str.split(',')
        .map(|id| match id.parse::<u64>() {
            Ok(id) => {
                if let Some(non_zero) = NonZeroU64::new(id) {
                    Ok(GuildId::from(non_zero))
                } else {
                    anyhow::bail!("0 is not a valid guild id")
                }
            }
            Err(e) => anyhow::bail!(e),
        })
        .collect()
}
