use serenity::all::{Attachment, User};

use crate::{
    assert_user, assert_user_server,
    database::badactor_model_controller::BadActorType,
    util::{locks::lock_user_id, logger::Logger},
    Context,
};

/// Subcommands for server configs.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands(
        "report",
        "deactivate",
        "reactivate",
        "display_latest",
        "display_by_user",
        "add_screenshot",
        "replace_screenshot",
        "update_explanation"
    ),
    subcommand_required
)]
pub async fn badactor(_: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn report(
    ctx: Context<'_>,
    user: User,
    actor_type: BadActorType,
    screenshot: Option<Attachment>,
    explanation: Option<String>,
) -> anyhow::Result<()> {
    assert_user_server!(ctx);

    let logger = Logger::get();

    ctx.defer().await?;

    {
        let _guard = lock_user_id(user.id).await;
        // do what needs to be protected
    }

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn deactivate(ctx: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn reactivate(ctx: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn display_latest(ctx: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn display_by_user(ctx: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn add_screenshot(ctx: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn replace_screenshot(ctx: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn update_explanation(ctx: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}
