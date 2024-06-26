use std::str::FromStr;

use ::serenity::all::CacheHttp;
use poise::serenity_prelude as serenity;
use poise::CreateReply;
use serenity::{CreateEmbed, GuildId, User};
use sqlx::PgPool;

use crate::assert_user;
use crate::database::controllers::scores_model_controller::ScoresModelController;
use crate::util::{embeds, format};
use crate::AppContext;

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

    let guild = GuildId::from_str(&server_id)?.to_partial_guild(ctx).await?;
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

    let embed = build_leaderboard(&ctx, &ctx.data().db_pool, ctx.author(), scoreboard_type).await?;
    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

async fn build_leaderboard(
    cache_http: impl CacheHttp,
    db_pool: &PgPool,
    interaction_user: &User,
    scoreboard_type: ScoreboardType,
) -> anyhow::Result<CreateEmbed> {
    let mut leaderboard: Vec<String> = Vec::new();

    let scores = match scoreboard_type {
        ScoreboardType::Users => ScoresModelController::get_top_users(db_pool, 10).await?,
        ScoreboardType::Servers => ScoresModelController::get_top_guilds(db_pool, 10).await?,
    };

    for (i, s) in scores.into_iter().enumerate() {
        if s.score == 0 {
            continue;
        }

        let display_user_or_guild = match scoreboard_type {
            ScoreboardType::Users => format!("<@{}>", s.id),
            ScoreboardType::Servers => {
                let guild_res = GuildId::from(s.id).to_partial_guild(&cache_http).await;
                match guild_res {
                    Ok(guild) => guild.name,
                    Err(_) => s.id.to_string(),
                }
            }
        };

        leaderboard.push(format!("{}. {}: {}", i + 1, display_user_or_guild, s.score))
    }

    let title = match scoreboard_type {
        ScoreboardType::Users => "Top 10 Users with the most reports",
        ScoreboardType::Servers => "Top 10 Servers with the most reports",
    };

    let embed = embeds::CreateJanitorEmbed::new(interaction_user)
        .into_embed()
        .title(title)
        .description(leaderboard.join("\n"));

    Ok(embed)
}
