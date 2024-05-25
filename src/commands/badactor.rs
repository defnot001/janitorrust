use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::{
    Attachment, ButtonStyle, ComponentInteraction, ComponentInteractionCollector, CreateActionRow,
    CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse, PartialGuild, User,
};

use crate::database::controllers::badactor_model_controller::{
    BadActor, BadActorModelController, BadActorType, CreateBadActorOptions,
};
use crate::database::controllers::scores_model_controller::ScoresModelController;
use crate::database::controllers::user_model_controller::UserModelController;
use crate::oops;
use crate::util::random_utils;
use crate::util::{embeds, format, locks, screenshot};
use crate::{Context as AppContext, Logger};
use crate::broadcast::broadcast;

enum ReportOutcome {
    Success,
    Fail,
    Cancel,
    Confirm,
}

struct CollectorOptions<'a> {
    ctx: AppContext<'a>,
    target_user: &'a User,
    collector: &'a ComponentInteraction,
    screenshot: Option<Attachment>,
    actor_type: BadActorType,
    explanation: Option<String>,
    interaction_guild: &'a PartialGuild,
}

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
        if !user.guild_ids.contains(&interaction_guild.id) {
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
        let options = CollectorOptions {
            ctx,
            target_user: &target_user,
            collector: &collector,
            screenshot,
            actor_type,
            explanation,
            interaction_guild: &interaction_guild,
        };

        return handle_collector(options).await;
    }

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn deactivate(_ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn reactivate(_ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn display_latest(_ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn display_by_user(_ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn add_screenshot(_ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn replace_screenshot(_ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn update_explanation(_ctx: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

async fn handle_collector(options: CollectorOptions<'_>) -> anyhow::Result<()> {
    let CollectorOptions {
        ctx,
        target_user,
        collector,
        screenshot,
        actor_type,
        explanation,
        interaction_guild,
    } = options;

    if collector.data.custom_id.as_str() == "cancel" {
        return respond_outcome(ctx, target_user, collector, ReportOutcome::Cancel).await;
    }

    if collector.data.custom_id.as_str() == "confirm" {
        respond_outcome(ctx, target_user, collector, ReportOutcome::Confirm).await?;

        let maybe_file_name = if let Some(screenshot) = screenshot {
            Some(save_screenshot(ctx, collector, screenshot, target_user).await?)
        } else {
            None
        };

        let options = CreateBadActorOptions {
            user_id: target_user.id,
            actor_type,
            screenshot_proof: maybe_file_name,
            explanation,
            updated_by_user_id: ctx.author().id,
            origin_guild_id: interaction_guild.id,
        };

        let bad_actor = save_bad_actor(ctx, target_user, collector, options).await?;

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
                format::display(interaction_guild)
            );
            Logger::get().error(ctx, e, log_msg).await;
        }

        let broadcast_options = broadcast::BroadcastOptions {
            ctx,
            target_user,
            bad_actor: &bad_actor,
            interaction_guild,
            broadcast_type: broadcast::BroadcastType::Report,
        };

        match broadcast::broadcast(broadcast_options).await {
            Ok(_) => {
                return respond_outcome(ctx, target_user, collector, ReportOutcome::Success).await;
            }
            Err(e) => {
                let log_msg = format!(
                    "Failed to broadcast the new report for {} to the community.",
                    format::display(target_user)
                );
                Logger::get().error(ctx, e, log_msg).await;

                return respond_outcome(ctx, target_user, collector, ReportOutcome::Fail).await;
            }
        }
    }

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

async fn respond_outcome(
    ctx: AppContext<'_>,
    target_user: &User,
    collector: &ComponentInteraction,
    outcome: ReportOutcome,
) -> anyhow::Result<()> {
    let message = match outcome {
        ReportOutcome::Cancel => format!(
            "Cancelled reporting user {}!",
            format::fdisplay(target_user)
        ),
        ReportOutcome::Confirm => format!(
            "Reporting user {} to the community and taking action...",
            format::fdisplay(target_user)
        ),
        ReportOutcome::Success => format!(
            "Successfully reported {} to the community!",
            format::fdisplay(target_user)
        ),
        ReportOutcome::Fail => format!(
            "Failed to report {} to the community!",
            format::fdisplay(target_user)
        ),
    };

    let response = CreateInteractionResponse::UpdateMessage(
        CreateInteractionResponseMessage::new()
            .content(message)
            .components(vec![]),
    );

    collector.create_response(&ctx, response).await?;

    Ok(())
}

async fn save_screenshot(
    ctx: AppContext<'_>,
    collector: &ComponentInteraction,
    screenshot: Attachment,
    target_user: &User,
) -> anyhow::Result<String> {
    let save_result = screenshot::FileManager::save(screenshot, target_user.id).await;

    match save_result {
        Ok(saved) => {
            tracing::info!("Screenshot {saved} saved.");
            Ok(saved)
        }
        Err(e) => {
            let log_msg = format!(
                "Failed to save screenshot for {}",
                format::display(target_user)
            );
            Logger::get().error(ctx, &e, &log_msg).await;

            let user_msg = format!(
                "Failed to save screenshot for {}!",
                format::fdisplay(target_user)
            );

            collector
                .edit_response(&ctx, EditInteractionResponse::default().content(user_msg))
                .await?;

            Err(e)
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
            Logger::get().error(ctx, &e, &log_msg).await;

            let user_msg = format!(
                "Failed to add bad actor {} to the dabase!",
                format::fdisplay(target_user)
            );
            collector
                .edit_response(&ctx, EditInteractionResponse::default().content(user_msg))
                .await?;

            Err(e)
        }
    }
}
