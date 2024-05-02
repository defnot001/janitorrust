use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::{
    Attachment, ButtonStyle, ComponentInteraction, ComponentInteractionCollector, CreateActionRow,
    CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse, User,
};

use crate::database::badactor_model_controller::{
    BadActor, BadActorModelController, BadActorType, CreateBadActorOptions,
};
use crate::database::scores_model_controller::ScoresModelController;
use crate::database::user_model_controller::UserModelController;
use crate::oops;
use crate::util::random_utils;
use crate::util::{embeds, format, locks, screenshot};
use crate::{Context as AppContext, Logger};

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
pub async fn badactor(_: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

/// Report a user for being naughty.
#[poise::command(slash_command, guild_only = true)]
pub async fn report(
    ctx: AppContext<'_>,
    #[description = "The user to report. You can also paste their id here."] target_user: User,
    #[description = "The type of bad act the user did."] actor_type: BadActorType,
    #[description = "A screenshot of the bad act. You can upload a file here."] screenshot: Option<
        Attachment,
    >,
    #[description = "If you can't provide a screenshot, please explain what happened here."]
    explanation: Option<String>,
) -> anyhow::Result<()> {
    let logger = Logger::get();

    let Some(interaction_guild) = ctx.partial_guild().await else {
        let user_msg = "This command can only be used in a server!";
        oops!(ctx, user_msg);
    };

    ctx.defer().await?;

    let user = match UserModelController::get(&ctx.data().db_pool, ctx.author().id).await {
        Ok(user) => user,
        Err(e) => {
            let log_msg = format!(
                "Failed to get interaction user {} from the database",
                format::display(ctx.author())
            );
            logger.error(ctx, e, log_msg).await;

            let user_msg = "You do not have permission to use this command!";
            oops!(ctx, user_msg);
        }
    };

    if let Some(user) = user {
        if !user.servers.contains(&interaction_guild.id) {
            let user_msg = "You can only use this command in one of your servers!";
            oops!(ctx, user_msg);
        }
    } else {
        let user_msg = "You do not have permission to use this command!";
        oops!(ctx, user_msg);
    }

    if screenshot.is_none() && explanation.is_none() {
        let user_msg = "You have to provide either a screenshot or an explanation.";
        oops!(ctx, user_msg);
    }

    let _guard = locks::lock_user_id(target_user.id).await;

    if BadActorModelController::has_active_case(&ctx.data().db_pool, target_user.id).await {
        let user_msg = format!(
            "User {} already has an active case!",
            format::fdisplay(&target_user)
        );
        oops!(ctx, user_msg);
    }

    ctx.send(get_check_user_reply(ctx, &target_user)).await?;

    if let Some(collector) = get_component_collector(ctx).await {
        if collector.data.custom_id.as_str() == "cancel" {
            return respond_cancel(ctx, &target_user, &collector).await;
        }

        if collector.data.custom_id.as_str() == "confirm" {
            respond_confirm(ctx, &target_user, &collector).await?;

            let file_name = match screenshot {
                Some(s) => {
                    let Ok(file_name) = save_screenshot(ctx, &collector, s, &target_user).await
                    else {
                        return Ok(());
                    };

                    Some(file_name)
                }
                None => None,
            };

            let options = CreateBadActorOptions {
                user_id: target_user.id,
                actor_type,
                screenshot_proof: file_name,
                explanation,
                last_changed_by: ctx.author().id,
                originally_created_in: interaction_guild.id,
            };

            let Ok(_) = save_bad_actor(ctx, &target_user, &collector, options).await else {
                return Ok(());
            };

            if let Err(e) = ScoresModelController::create_or_increase_scoreboards(
                &ctx.data().db_pool,
                ctx.author().id,
                interaction_guild.id,
            )
            .await
            {
                let log_msg = format!(
                    "Failed to updated scores for user {} or guild {}",
                    format::display(ctx.author()),
                    format::display(&interaction_guild)
                );
                logger.error(ctx, e, log_msg).await;
            }

            // match broadcast_bad_actor().await {
            //     Ok(_) => {
            //         return respond_success(&ctx, &target_user, &collector).await;
            //     }
            //     Err(e) => {
            //         let log_msg = format!(
            //             "Failed to broadcast the new report for {} to the community.",
            //             display(&target_user)
            //         );
            //         logger.error(&ctx, e, log_msg).await;

            //         return respond_failure(&ctx, &target_user, &collector).await;
            //     }
            // }
        }
    }

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn deactivate(ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn reactivate(ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn display_latest(ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn display_by_user(ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn add_screenshot(ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn replace_screenshot(ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn update_explanation(ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

fn get_check_user_reply(ctx: AppContext<'_>, target_user: &User) -> CreateReply {
    let created_at = format::display_time(target_user.created_at().to_utc());

    let bad_actor_user_embed = embeds::CreateJanitorEmbed::new(ctx.author())
        .avatar_thumbnail(target_user)
        .into_embed()
        .title(format!("Info User {}", random_utils::username(target_user)))
        .field("ID", target_user.id.to_string(), false)
        .field("Created At", created_at, false);

    let action_row = CreateActionRow::Buttons(vec![
        CreateButton::new("confirm")
            .label("Confirm")
            .style(ButtonStyle::Success),
        CreateButton::new("cancel")
            .label("Cancel")
            .style(ButtonStyle::Danger),
    ]);

    CreateReply::default()
        .components(vec![action_row])
        .content("Is this the user that you want to report?")
        .embed(bad_actor_user_embed)
}

async fn get_component_collector(ctx: AppContext<'_>) -> Option<ComponentInteraction> {
    ComponentInteractionCollector::new(ctx)
        .author_id(ctx.author().id)
        .channel_id(ctx.channel_id())
        .timeout(std::time::Duration::from_secs(120))
        .await
        .filter(move |c| {
            c.data.custom_id.as_str() == "confirm" || c.data.custom_id.as_str() == "cancel"
        })
}

async fn update_component_response(
    ctx: AppContext<'_>,
    target_user: &User,
    collector: &ComponentInteraction,
    message: String,
) -> anyhow::Result<()> {
    let response = CreateInteractionResponse::UpdateMessage(
        CreateInteractionResponseMessage::new()
            .content(message)
            .components(vec![]),
    );

    collector.create_response(&ctx, response).await?;
    Ok(())
}

async fn respond_cancel(
    ctx: AppContext<'_>,
    target_user: &User,
    collector: &ComponentInteraction,
) -> anyhow::Result<()> {
    let message = format!(
        "Cancelled reporting user {}!",
        format::fdisplay(target_user)
    );
    update_component_response(ctx, target_user, collector, message).await
}

async fn respond_confirm(
    ctx: AppContext<'_>,
    target_user: &User,
    collector: &ComponentInteraction,
) -> anyhow::Result<()> {
    let message = format!(
        "Reporting user {} to the community and taking action...",
        format::fdisplay(target_user)
    );
    update_component_response(ctx, target_user, collector, message).await
}

async fn respond_success(
    ctx: AppContext<'_>,
    target_user: &User,
    collector: &ComponentInteraction,
) -> anyhow::Result<()> {
    let message = format!(
        "Successfully reported {} to the community!",
        format::fdisplay(target_user)
    );
    update_component_response(ctx, target_user, collector, message).await
}

async fn respond_failure(
    ctx: AppContext<'_>,
    target_user: &User,
    collector: &ComponentInteraction,
) -> anyhow::Result<()> {
    let message = format!(
        "Failed to report {} to the community!",
        format::fdisplay(target_user)
    );
    update_component_response(ctx, target_user, collector, message).await
}

async fn save_screenshot(
    ctx: AppContext<'_>,
    collector: &ComponentInteraction,
    screenshot: Attachment,
    target_user: &User,
) -> anyhow::Result<String> {
    match screenshot::FileManager::save(screenshot, target_user.id).await {
        Ok(saved) => Ok(saved),
        Err(e) => {
            let log_msg = format!(
                "Failed to save screenshot for {}",
                format::display(target_user)
            );
            Logger::get().error(ctx, e, &log_msg).await;

            let user_msg = format!(
                "Failed to save screenshot for {}!",
                format::fdisplay(target_user)
            );
            collector
                .edit_response(&ctx, EditInteractionResponse::default().content(user_msg))
                .await?;

            anyhow::bail!(log_msg);
        }
    }
}

async fn save_bad_actor(
    ctx: AppContext<'_>,
    target_user: &User,
    collector: &ComponentInteraction,
    options: CreateBadActorOptions,
) -> anyhow::Result<BadActor> {
    match BadActorModelController::create(&ctx.data().db_pool, options).await {
        Ok(bad_actor) => Ok(bad_actor),
        Err(e) => {
            let log_msg = format!(
                "Failed to add bad actor {} to the dabase",
                format::display(target_user)
            );
            Logger::get().error(ctx, e, &log_msg).await;

            let user_msg = format!(
                "Failed to add bad actor {} to the dabase!",
                format::fdisplay(target_user)
            );
            collector
                .edit_response(&ctx, EditInteractionResponse::default().content(user_msg))
                .await?;

            anyhow::bail!(log_msg);
        }
    }
}
