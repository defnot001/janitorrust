use futures::future;
use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::{
    Attachment, ButtonStyle, ComponentInteraction, ComponentInteractionCollector, CreateActionRow,
    CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse, PartialGuild, User,
};

use crate::assert_user_server;
use crate::broadcast::broadcast_handler;
use crate::database::controllers::badactor_model_controller::BroadcastEmbedOptions;
use crate::database::controllers::badactor_model_controller::{
    BadActor, BadActorModelController, BadActorQueryType, BadActorType, CreateBadActorOptions,
};
use crate::database::controllers::scores_model_controller::ScoresModelController;
use crate::util::random_utils;
use crate::util::{embeds, format, locks, screenshot};
use crate::{AppContext, Logger};

enum ReportOutcome {
    Success,
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
    interaction_guild: PartialGuild,
}

/// Subcommands for server configs.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands(
        "report",
        "deactivate",
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
    #[description = "The user to report. You can also paste their ID here."] target_user: User,
    #[description = "The type of bad act the user did."] actor_type: BadActorType,
    #[description = "A screenshot of the bad act. You can upload a file here."] screenshot: Option<
        Attachment,
    >,
    #[description = "If you can't provide a screenshot, please explain what happened here."]
    explanation: Option<String>,
) -> anyhow::Result<()> {
    ctx.defer().await?;

    let Some(interaction_guild) = ctx.partial_guild().await else {
        ctx.say("This command can only be used in a server!")
            .await?;
        return Ok(());
    };

    assert_user_server!(ctx);

    if screenshot.is_none() && explanation.is_none() {
        ctx.say("You have to provide either a screenshot or an explanation.")
            .await?;
        return Ok(());
    }

    let _guard = locks::lock_user_id(target_user.id).await;

    if BadActorModelController::has_active_case(&ctx.data().db_pool, target_user.id).await {
        ctx.say(format!(
            "User {} already has an active case!",
            format::fdisplay(&target_user)
        ))
        .await?;
        return Ok(());
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
            interaction_guild,
        };

        return handle_collector(options).await;
    }

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn deactivate(
    ctx: AppContext<'_>,
    #[description = "The ID of the report that you want to deactivate."] report_id: u64,
    #[description = "Reason for deactivating the report"] explanation: String,
) -> anyhow::Result<()> {
    ctx.defer().await?;

    let Some(interaction_guild) = ctx.partial_guild().await else {
        ctx.say("This command can only be used in a server!")
            .await?;
        return Ok(());
    };

    assert_user_server!(ctx);

    let old_entry = BadActorModelController::get_by_id(&ctx.data().db_pool, report_id).await?;

    if let Some(entry) = old_entry {
        if !entry.is_active {
            ctx.say("This entry is not active!").await?;
            return Ok(());
        }
    } else {
        ctx.say("There is no such entry in the database!").await?;
        return Ok(());
    }

    let deactivated = BadActorModelController::deavtivate(
        &ctx.data().db_pool,
        report_id,
        explanation,
        ctx.author().id,
    )
    .await?;

    let Some(target_user) = deactivated.user(ctx).await else {
        let log_msg = format!(
            "User with ID {} does not exist anymore, skipping broadcast",
            deactivated.user_id
        );
        Logger::get().warn(ctx, log_msg).await;

        ctx.say("This user's account no longer exists, deactivating it does not have any impact.")
            .await?;
        return Ok(());
    };

    let origin_guild_id = interaction_guild.id;
    let broadcast_options = broadcast_handler::BroadcastOptions {
        bad_actor: &deactivated,
        bad_actor_user: &target_user,
        reporting_user: ctx.author(),
        broadcast_type: broadcast_handler::BroadcastType::Deactivate,
        config: &ctx.data().config,
        db_pool: &ctx.data().db_pool,
        origin_guild: &Some(interaction_guild),
        origin_guild_id,
        reporting_bot_id: ctx.framework().bot_id,
    };

    broadcast_handler::broadcast(&ctx, broadcast_options).await;

    ctx.say(format!("Successfully disabled report entry {report_id}."))
        .await?;

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn display_latest(
    ctx: AppContext<'_>,
    #[description = "The amount of entries you want to display. Max 10. Defaults to 5."]
    limit: Option<u8>,
    #[description = "The type of reports you want to display. Defaults to all report types."]
    report_type: Option<BadActorQueryType>,
) -> anyhow::Result<()> {
    ctx.defer().await?;
    assert_user_server!(ctx);

    let mut limit = limit.unwrap_or(5);

    if limit > 10 {
        limit = 10;
    }

    let latest =
        BadActorModelController::get_by_type(&ctx.data().db_pool, limit, report_type).await?;

    let reply = construct_embeds_message(ctx, latest).await;
    ctx.send(reply).await?;

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn display_by_user(
    ctx: AppContext<'_>,
    #[description = "The user to display the reports from. You can also paste their ID here."]
    target_user: User,
) -> anyhow::Result<()> {
    ctx.defer().await?;
    assert_user_server!(ctx);

    let entries =
        BadActorModelController::get_by_user_id(&ctx.data().db_pool, target_user.id).await?;

    if entries.is_empty() {
        ctx.say(format!(
            "User {} does not have any entries.",
            format::fdisplay(&target_user)
        ))
        .await?;
        return Ok(());
    }

    let reply = construct_embeds_message(ctx, entries).await;
    ctx.send(reply).await?;

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn add_screenshot(
    ctx: AppContext<'_>,
    #[description = "The report ID you want to add the screenshot to."] report_id: u64,
    #[description = "The screenshot you want to add. You can upload a file here."]
    screenshot: Attachment,
) -> anyhow::Result<()> {
    ctx.defer().await?;

    let Some(interaction_guild) = ctx.partial_guild().await else {
        ctx.say("This command can only be used in a server!")
            .await?;
        return Ok(());
    };

    assert_user_server!(ctx);

    let old_entry = match BadActorModelController::get_by_id(&ctx.data().db_pool, report_id).await?
    {
        Some(old) => {
            if old.screenshot_proof.is_some() {
                ctx.say("This report ID already has a screenshot proof. Please use `/badactor replace_screenshot` if you want to overwrite it.").await?;
                return Ok(());
            }

            old
        }
        None => {
            ctx.say("There is no entry with this report ID!").await?;
            return Ok(());
        }
    };

    let screenshot_path = match screenshot::FileManager::save(screenshot, old_entry.user_id).await {
        Ok(path) => path,
        Err(e) => {
            let log_msg = "Failed to save screenshot";
            Logger::get().error(ctx, e, log_msg).await;

            ctx.say("Failed to save screenshot!").await?;
            return Ok(());
        }
    };

    let updated = BadActorModelController::update_screenshot(
        &ctx.data().db_pool,
        report_id,
        ctx.author().id,
        screenshot_path,
    )
    .await?;

    let Some(target_user) = updated.user(ctx).await else {
        let log_msg = format!(
            "User with ID {} does not exist anymore, skipping broadcast",
            updated.user_id
        );
        Logger::get().warn(ctx, log_msg).await;

        ctx.say("This user's account no longer exists. The screenshot was updated in the database but broadcasting will be skipped.")
            .await?;
        return Ok(());
    };

    let origin_guild_id = interaction_guild.id;
    let broadcast_options = broadcast_handler::BroadcastOptions {
        bad_actor: &updated,
        bad_actor_user: &target_user,
        reporting_user: ctx.author(),
        broadcast_type: broadcast_handler::BroadcastType::AddScreenshot,
        config: &ctx.data().config,
        db_pool: &ctx.data().db_pool,
        origin_guild: &Some(interaction_guild),
        origin_guild_id,
        reporting_bot_id: ctx.framework().bot_id,
    };

    broadcast_handler::broadcast(&ctx, broadcast_options).await;

    ctx.say(format!(
        "Successfully updated screenshot for report entry {report_id}."
    ))
    .await?;

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn replace_screenshot(
    ctx: AppContext<'_>,
    #[description = "The report ID you want to replace the screenshot of."] report_id: u64,
    #[description = "The screenshot you want replace the old one with. You can upload a file here."]
    screenshot: Attachment,
) -> anyhow::Result<()> {
    ctx.defer().await?;

    let Some(interaction_guild) = ctx.partial_guild().await else {
        ctx.say("This command can only be used in a server!")
            .await?;
        return Ok(());
    };

    assert_user_server!(ctx);

    let old_entry = match BadActorModelController::get_by_id(&ctx.data().db_pool, report_id).await?
    {
        Some(old) => {
            if old.screenshot_proof.is_none() {
                ctx.say("This report ID does not have a screenshot proof yet. Please use `/badactor add_screenshot` if you want to provide one for it.").await?;
                return Ok(());
            }

            old
        }
        None => {
            ctx.say("There is no entry with this report ID!").await?;
            return Ok(());
        }
    };

    let old_path = old_entry.screenshot_proof.unwrap();

    let new_path = match screenshot::FileManager::save(screenshot, old_entry.user_id).await {
        Ok(path) => path,
        Err(e) => {
            let log_msg = "Failed to save screenshot";
            Logger::get().error(ctx, e, log_msg).await;

            ctx.say("Failed to save screenshot!").await?;
            return Ok(());
        }
    };

    screenshot::FileManager::delete(&old_path).await?;

    let updated = BadActorModelController::update_screenshot(
        &ctx.data().db_pool,
        report_id,
        ctx.author().id,
        new_path,
    )
    .await?;

    let Some(target_user) = updated.user(ctx).await else {
        let log_msg = format!(
            "User with ID {} does not exist anymore, skipping broadcast",
            updated.user_id
        );
        Logger::get().warn(ctx, log_msg).await;

        ctx.say("This user's account no longer exists. The screenshot was updated in the database but broadcasting will be skipped.")
            .await?;
        return Ok(());
    };

    let origin_guild_id = interaction_guild.id;
    let broadcast_options = broadcast_handler::BroadcastOptions {
        bad_actor: &updated,
        bad_actor_user: &target_user,
        reporting_user: ctx.author(),
        broadcast_type: broadcast_handler::BroadcastType::ReplaceScreenshot,
        config: &ctx.data().config,
        db_pool: &ctx.data().db_pool,
        origin_guild: &Some(interaction_guild),
        origin_guild_id,
        reporting_bot_id: ctx.framework().bot_id,
    };

    broadcast_handler::broadcast(&ctx, broadcast_options).await;

    ctx.say(format!(
        "Successfully updated screenshot for report entry {report_id}."
    ))
    .await?;

    Ok(())
}

#[poise::command(slash_command, guild_only = true)]
pub async fn update_explanation(
    ctx: AppContext<'_>,
    #[description = "The report ID you want to replace the screenshot of."] report_id: u64,
    #[description = "The updated explanation you want to provide for the report."]
    explanation: String,
) -> anyhow::Result<()> {
    ctx.defer().await?;

    let Some(interaction_guild) = ctx.partial_guild().await else {
        ctx.say("This command can only be used in a server!")
            .await?;
        return Ok(());
    };

    assert_user_server!(ctx);

    let updated = BadActorModelController::update_explanation(
        &ctx.data().db_pool,
        report_id,
        ctx.author().id,
        explanation,
    )
    .await?;

    let Some(target_user) = updated.user(ctx).await else {
        let log_msg = format!(
            "User with ID {} does not exist anymore, skipping broadcast",
            updated.user_id
        );
        Logger::get().warn(ctx, log_msg).await;

        ctx.say("This user's account no longer exists. The explanation was updated in the database but broadcasting will be skipped.")
            .await?;
        return Ok(());
    };

    let origin_guild_id = interaction_guild.id;
    let broadcast_options = broadcast_handler::BroadcastOptions {
        bad_actor: &updated,
        bad_actor_user: &target_user,
        reporting_user: ctx.author(),
        broadcast_type: broadcast_handler::BroadcastType::UpdateExplanation,
        config: &ctx.data().config,
        db_pool: &ctx.data().db_pool,
        origin_guild: &Some(interaction_guild),
        origin_guild_id,
        reporting_bot_id: ctx.framework().bot_id,
    };

    broadcast_handler::broadcast(&ctx, broadcast_options).await;

    ctx.say(format!(
        "Successfully updated explanation for report entry {report_id}."
    ))
    .await?;

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
                format::display(&interaction_guild)
            );
            Logger::get().error(ctx, e, log_msg).await;
        }

        let origin_guild_id = interaction_guild.id;
        let broadcast_options = broadcast_handler::BroadcastOptions {
            bad_actor: &bad_actor,
            bad_actor_user: target_user,
            reporting_user: ctx.author(),
            broadcast_type: broadcast_handler::BroadcastType::Report,
            config: &ctx.data().config,
            db_pool: &ctx.data().db_pool,
            origin_guild: &Some(interaction_guild),
            origin_guild_id,
            reporting_bot_id: ctx.framework().bot_id,
        };

        broadcast_handler::broadcast(&ctx, broadcast_options).await;
        return respond_outcome(ctx, target_user, collector, ReportOutcome::Success).await;
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

/// Returns the [CreateReply] built from the vector of [BadActor]s.
/// This checks for empty vectors or more than 10 embeds and returns error messages if those conditions are violated.
async fn construct_embeds_message(ctx: AppContext<'_>, bad_actors: Vec<BadActor>) -> CreateReply {
    if bad_actors.is_empty() {
        return CreateReply::default().content("There are no bad actor entries to display!");
    }

    if bad_actors.len() > 10 {
        return CreateReply::default().content("Only 10 entries can be displayed at one time!");
    }

    let iter = bad_actors.into_iter().map(|b| async move {
        let guild = b.origin_guild_id.to_partial_guild(ctx).await.ok();

        let embed_options = BroadcastEmbedOptions {
            bot_id: ctx.framework().bot_id,
            origin_guild: &guild,
            origin_guild_id: b.origin_guild_id,
            report_author: ctx.author(),
        };

        b.to_broadcast_embed(ctx, embed_options).await
    });

    let joined = future::join_all(iter).await;
    let mut embeds = Vec::with_capacity(joined.len());
    let mut attachments = Vec::with_capacity(joined.len());

    for (embed, attachment) in joined {
        embeds.push(embed);

        if let Some(a) = attachment {
            attachments.push(a);
        }
    }

    CreateReply {
        embeds,
        attachments,
        ..Default::default()
    }
}
