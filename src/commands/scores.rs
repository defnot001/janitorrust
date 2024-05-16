use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::{CreateEmbed, GuildId, User, UserId};

use crate::assert_user;
use crate::database::scores_model_controller::{Scoreboard, ScoresModelController};
use crate::util::{embeds, format, random_utils};
use crate::Context as AppContext;

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
pub async fn scores(_: AppContext<'_>) -> anyhow::Result<()> {
    Ok(())
}

/// Check the report score of a server.
#[poise::command(slash_command, guild_only = true)]
async fn server(
    ctx: AppContext<'_>,
    #[description = "The ID of the guild you want to get the scores for."] server_id: String,
) -> anyhow::Result<()> {
    assert_user!(ctx);
    ctx.defer().await?;

    let guild = GuildId::from(random_utils::parse_snowflake(&server_id)?)
        .to_partial_guild(ctx)
        .await?;

    let scores = ScoresModelController::get_guild_score(&ctx.data().db_pool, guild.id).await?;

    if scores.score == 0 {
        let reply = format!(
            "Admins from {} have not created any reports for bad actors yet.",
            format::fdisplay(&guild)
        );
        ctx.say(reply).await?;
        return Ok(());
    }

    let reply = format!(
        "Admins from {} have reported {} bad actors. Thank you for keeping the community safe!",
        format::fdisplay(&guild),
        scores.score
    );

    ctx.say(reply).await?;
    Ok(())
}

/// Check the report score of a user.
#[poise::command(slash_command, guild_only = true)]
async fn user(
    ctx: AppContext<'_>,
    #[description = "The User that you want to see the scores for."] user: User,
) -> anyhow::Result<()> {
    assert_user!(ctx);
    ctx.defer().await?;

    let user_scores = ScoresModelController::get_user_score(&ctx.data().db_pool, user.id).await?;

    let reply = match user_scores.score {
        0 => format!(
            "User {} has not created any reports for bad actors yet.",
            format::fdisplay(&user)
        ),
        1..=20 => format!(
            "User {} has reported {} bad actors so far. Keep up the good work!",
            format::fdisplay(&user),
            user_scores.score
        ),
        21.. => format!(
            "User {} has reported {} bad actors so far. What a hero!",
            format::fdisplay(&user),
            user_scores.score
        ),
    };

    ctx.say(reply).await?;
    Ok(())
}

/// Check the leaderboards
#[poise::command(slash_command, guild_only = true)]
async fn leaderboard(
    ctx: AppContext<'_>,
    #[description = "The type of scoreboard you want."] scoreboard_type: ScoreboardType,
) -> anyhow::Result<()> {
    assert_user!(ctx);
    ctx.defer().await?;

    let embed = match scoreboard_type {
        ScoreboardType::Users => {
            let scores = ScoresModelController::get_top_users(&ctx.data().db_pool, 10).await?;
            build_user_leaderboard(scores, ctx.author(), ctx).await?
        }
        ScoreboardType::Servers => {
            let scores = ScoresModelController::get_top_guilds(&ctx.data().db_pool, 10).await?;
            build_guilds_leaderboard(scores, ctx.author(), ctx).await?
        }
    };

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

async fn build_user_leaderboard(
    scores: Vec<Scoreboard>,
    interaction_user: &User,
    ctx: AppContext<'_>,
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
            format::user_mention(&user.id),
            s.score
        ))
    }

    if leaderboard.is_empty() {
        anyhow::bail!("Cannot build embed from empty vector");
    }

    let embed = embeds::CreateJanitorEmbed::new(interaction_user)
        .into_embed()
        .title("Top 10 Users with the most reports")
        .description(leaderboard.join("\n"));

    Ok(embed)
}

async fn build_guilds_leaderboard(
    scores: Vec<Scoreboard>,
    interaction_user: &User,
    ctx: AppContext<'_>,
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

    let embed = embeds::CreateJanitorEmbed::new(interaction_user)
        .into_embed()
        .title("Top 10 Guilds with the most reports")
        .description(leaderboard.join("\n"));

    Ok(embed)
}
