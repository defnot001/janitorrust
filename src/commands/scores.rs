use std::num::NonZeroU64;

use poise::CreateReply;
use serenity::all::{CreateEmbed, GuildId, User, UserId};
use sqlx::PgPool;

use crate::{
    assert_user,
    database::{
        scores_model_controller::{Scoreboard, ScoresModelController},
        serverconfig_model_controller::ServerConfigModelController,
    },
    util::{
        builders::create_default_embed,
        error::{respond_error, respond_mistake},
        format::{display, fdisplay, user_mention},
        random_utils::parse_snowflake,
    },
    Context,
};

#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
enum ScoreboardType {
    Users,
    Servers,
}

/// Subcommands for scores.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands("server", "user", "leaderboard"),
    subcommand_required
)]
pub async fn scores(_: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

/// Check the report score of a server.
#[poise::command(slash_command, guild_only = true)]
async fn server(
    ctx: Context<'_>,
    #[description = "The ID of the guild you want to get the scores for."] guild_id: String,
) -> anyhow::Result<()> {
    assert_user!(ctx);

    let guild_id = match parse_snowflake(guild_id) {
        Ok(id) => GuildId::from(id),
        Err(e) => {
            let msg = "Failed to parse the provided guild id";
            return respond_error(msg, e, &ctx).await;
        }
    };

    let guild = match guild_id.to_partial_guild(&ctx).await {
        Ok(guild) => guild,
        Err(e) => {
            let msg = "Failed to find the guild for the provided ID";
            return respond_error(msg, e, &ctx).await;
        }
    };

    let scores = match ScoresModelController::get_guild_score(&ctx.data().db_pool, guild_id).await {
        Ok(scores) => scores,
        Err(e) => {
            let msg = format!(
                "Failed to query the scores for {} from the database",
                display(&guild)
            );
            return respond_error(msg, e, &ctx).await;
        }
    };

    let scores = match scores {
        Some(scores) => scores,
        None => {
            let msg = format!(
                "{} does not have any scores in the database",
                display(&guild)
            );
            return respond_mistake(msg, &ctx).await;
        }
    };

    if scores.score == 0 {
        let msg = format!(
            "Admins from {} have not created any reports for bad actors yet!",
            fdisplay(&guild)
        );
        ctx.say(msg).await?;
        return Ok(());
    }

    let msg = format!(
        "Admins from {} have reported {} bad actors. Thank you for keeping the community safe!",
        fdisplay(&guild),
        scores.score
    );

    ctx.say(msg).await?;

    Ok(())
}

/// Check the report score of a user.
#[poise::command(slash_command, guild_only = true)]
async fn user(
    ctx: Context<'_>,
    #[description = "The User that you want to see the scores for."] user: User,
) -> anyhow::Result<()> {
    assert_user!(ctx);

    let user_scores =
        match ScoresModelController::get_user_score(&ctx.data().db_pool, user.id).await {
            Ok(scores) => scores,
            Err(e) => {
                let msg = format!(
                    "Failed query the scores for {} from the database",
                    display(&user)
                );
                return respond_error(msg, e, &ctx).await;
            }
        };

    let user_scores = match user_scores {
        Some(scores) => scores,
        None => {
            let msg = format!(
                "Cannot find the scores for {} in the database",
                display(&user)
            );
            return respond_mistake(msg, &ctx).await;
        }
    };

    let message = match user_scores.score {
        0 => format!(
            "User {} has not created any reports for bad actors yet.",
            fdisplay(&user)
        ),
        1..=20 => format!(
            "User {} has reported {} bad actors so far. Keep up the good work!",
            fdisplay(&user),
            user_scores.score
        ),
        21.. => format!(
            "User {} has reported {} bad actors so far. What a hero!",
            fdisplay(&user),
            user_scores.score
        ),
    };

    ctx.say(message).await?;

    Ok(())
}

/// Check the leaderboards
#[poise::command(slash_command, guild_only = true)]
async fn leaderboard(
    ctx: Context<'_>,
    #[description = "The type of scoreboard you want."] scoreboard_type: ScoreboardType,
) -> anyhow::Result<()> {
    assert_user!(ctx);

    let scores = match scoreboard_type {
        ScoreboardType::Users => {
            ScoresModelController::get_top_users(&ctx.data().db_pool, 10).await
        }
        ScoreboardType::Servers => {
            ScoresModelController::get_top_guilds(&ctx.data().db_pool, 10).await
        }
    };

    let scores = match scores {
        Ok(scores) => scores,
        Err(e) => {
            let msg = "Failed to query the scoreboard from the database";
            return respond_error(msg, e, &ctx).await;
        }
    };

    let embed = match scoreboard_type {
        ScoreboardType::Users => build_user_leaderboard(scores, ctx.author(), &ctx).await,
        ScoreboardType::Servers => build_guilds_leaderboard(scores, ctx.author(), &ctx).await,
    };

    match embed {
        Ok(embed) => {
            ctx.send(CreateReply::default().embed(embed)).await?;
            Ok(())
        }
        Err(e) => {
            let msg = "Failed to query the discord API to build the leaderboard embed";
            respond_error(msg, e, &ctx).await
        }
    }
}

async fn build_user_leaderboard(
    scores: Vec<Scoreboard>,
    interaction_user: &User,
    ctx: &Context<'_>,
) -> anyhow::Result<CreateEmbed> {
    let mut leaderboard = Vec::with_capacity(scores.len());

    for (i, s) in scores.into_iter().enumerate() {
        let user = UserId::from(s.discord_id).to_user(ctx).await?;

        if s.score == 0 {
            continue;
        }

        leaderboard.push(format!(
            "{}. {}: `{}`",
            i + 1,
            user_mention(&user.id),
            s.score
        ))
    }

    if leaderboard.is_empty() {
        anyhow::bail!("Cannot build embed from empty vector");
    }

    Ok(create_default_embed(interaction_user)
        .title("Top 10 Users with the most reports")
        .description(leaderboard.join("\n")))
}

async fn build_guilds_leaderboard(
    scores: Vec<Scoreboard>,
    interaction_user: &User,
    ctx: &Context<'_>,
) -> anyhow::Result<CreateEmbed> {
    let mut leaderboard = Vec::with_capacity(scores.len());

    for (i, s) in scores.into_iter().enumerate() {
        let guild = GuildId::from(s.discord_id).to_partial_guild(ctx).await?;

        if s.score == 0 {
            continue;
        }

        leaderboard.push(format!("{}. {}: `{}`", i + 1, guild.name, s.score));
    }

    if leaderboard.is_empty() {
        anyhow::bail!("Cannot build embed from empty vector");
    }

    Ok(create_default_embed(interaction_user)
        .title("Top 10 Guilds with the most reports")
        .description(leaderboard.join("\n")))
}
