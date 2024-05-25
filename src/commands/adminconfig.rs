use poise::CreateReply;

use crate::database::controllers::badactor_model_controller::BadActorModelController;
use crate::database::controllers::serverconfig_model_controller::{
    ServerConfigComplete, ServerConfigModelController,
};
use crate::util::random_utils::parse_guild_ids;
use crate::util::screenshot::FileManager;
use crate::Context as AppContext;
use crate::{assert_admin, assert_admin_server};

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
    ctx.defer().await?;

    let guild_ids = parse_guild_ids(&guild_id)?;

    let configs =
        ServerConfigModelController::get_multiple_by_guild_id(&ctx.data().db_pool, &guild_ids)
            .await?;

    let mut embeds = Vec::with_capacity(guild_ids.len());

    for config in configs {
        embeds.push(
            ServerConfigComplete::try_from_server_config(config, ctx)
                .await?
                .to_embed(ctx.author()),
        );
    }

    let reply = CreateReply {
        embeds,
        ..Default::default()
    };

    ctx.send(reply).await?;
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
    ctx.defer().await?;

    let deleted = BadActorModelController::delete(&ctx.data().db_pool, entry).await?;

    if let Some(file_name) = deleted.screenshot_proof.as_ref() {
        FileManager::delete(file_name).await?;
    }

    let reply = format!("Successfully deleted bad actor entry with id {entry} from the database.");

    ctx.say(reply).await?;
    Ok(())
}
