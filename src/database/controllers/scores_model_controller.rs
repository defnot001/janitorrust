use std::str::FromStr;

use anyhow::Context;
use poise::serenity_prelude as serenity;
use serenity::{GuildId, UserId};
use sqlx::{prelude::FromRow, PgPool};

#[derive(Debug, FromRow)]
struct DbUserScoreboard {
    user_id: String,
    score: i32,
}

#[derive(Debug)]
pub struct UserScoreboard {
    pub user_id: UserId,
    pub score: u32,
}

impl TryFrom<DbUserScoreboard> for UserScoreboard {
    type Error = anyhow::Error;

    fn try_from(db_user_scoreboard: DbUserScoreboard) -> Result<Self, Self::Error> {
        let user_id = UserId::from_str(&db_user_scoreboard.user_id)?;
        let score = db_user_scoreboard.score as u32;

        Ok(UserScoreboard { user_id, score })
    }
}

#[derive(Debug, FromRow)]
struct DbGuildScoreboard {
    guild_id: String,
    score: i32,
}

#[derive(Debug)]
pub struct GuildScoreboard {
    pub guild_id: GuildId,
    pub score: u32,
}

impl TryFrom<DbGuildScoreboard> for GuildScoreboard {
    type Error = anyhow::Error;

    fn try_from(db_guild_scoreboard: DbGuildScoreboard) -> Result<Self, Self::Error> {
        let guild_id = GuildId::from_str(&db_guild_scoreboard.guild_id)?;
        let score = db_guild_scoreboard.score as u32;

        Ok(GuildScoreboard { guild_id, score })
    }
}

pub struct ScoresModelController;

impl ScoresModelController {
    pub async fn create_or_increase_scoreboards(
        db_pool: &PgPool,
        user_id: UserId,
        guild_id: GuildId,
    ) -> anyhow::Result<()> {
        sqlx::query("BEGIN").execute(db_pool).await?;

        let user_res = sqlx::query(
            r#"
            INSERT INTO user_scores (user_id, score)
            VALUES ($1, 1)
            ON CONFLICT (user_id)
            DO UPDATE SET score = user_scores.score + 1;
            "#,
        )
        .bind(user_id.to_string())
        .execute(db_pool)
        .await;

        let guild_res = sqlx::query(
            r#"
            INSERT INTO guild_scores (guild_id, score)
            VALUES ($1, 1)
            ON CONFLICT (guild_id)
            DO UPDATE SET score = guild_scores.score + 1;
            "#,
        )
        .bind(guild_id.to_string())
        .execute(db_pool)
        .await;

        if user_res.is_err() || guild_res.is_err() {
            sqlx::query("ROLLBACK").execute(db_pool).await?;
            return Err(anyhow::anyhow!("Failed to create or increase scoreboards"));
        }

        sqlx::query("COMMIT").execute(db_pool).await?;

        Ok(())
    }

    pub async fn get_top_users(db_pool: &PgPool, limit: u8) -> anyhow::Result<Vec<UserScoreboard>> {
        let db_top_users = sqlx::query_as::<_, DbUserScoreboard>(
            r#"
            SELECT * FROM user_scores
            ORDER BY score DESC
            LIMIT $1;
            "#,
        )
        .bind(limit as i16)
        .fetch_all(db_pool)
        .await
        .context(format!(
            "Failed to get top {limit} users from the `user_scores` table"
        ))?;

        db_top_users
            .into_iter()
            .map(UserScoreboard::try_from)
            .collect::<Result<Vec<UserScoreboard>, _>>()
    }

    pub async fn get_top_guilds(
        db_pool: &PgPool,
        limit: u8,
    ) -> anyhow::Result<Vec<GuildScoreboard>> {
        let db_top_guilds = sqlx::query_as::<_, DbGuildScoreboard>(
            r#"
            SELECT * FROM guild_scores
            ORDER BY score DESC
            LIMIT $1;
            "#,
        )
        .bind(limit as i16)
        .fetch_all(db_pool)
        .await
        .context(format!(
            "Failed to get top {limit} guilds from the `guild_scores` table"
        ))?;

        db_top_guilds
            .into_iter()
            .map(GuildScoreboard::try_from)
            .collect::<Result<Vec<GuildScoreboard>, _>>()
    }

    pub async fn get_user_score(
        db_pool: &PgPool,
        user_id: UserId,
    ) -> anyhow::Result<UserScoreboard> {
        let db_score =
            sqlx::query_as::<_, DbUserScoreboard>("SELECT * FROM user_scores WHERE user_id = $1;")
                .bind(user_id.to_string())
                .fetch_optional(db_pool)
                .await
                .context(format!("Failed to get scores for user {user_id}"))?;

        match db_score {
            Some(db_score) => db_score.try_into(),
            None => Err(anyhow::anyhow!(
                "No scores for user {user_id} found in the `user_scores` table",
            )),
        }
    }

    pub async fn get_guild_score(
        db_pool: &PgPool,
        guild_id: GuildId,
    ) -> anyhow::Result<GuildScoreboard> {
        let db_score = sqlx::query_as::<_, DbGuildScoreboard>(
            "SELECT * FROM guild_scores WHERE guild_id = $1;",
        )
        .bind(guild_id.to_string())
        .fetch_optional(db_pool)
        .await
        .context(format!("Failed to get guild scores for guild {guild_id}"))?;

        match db_score {
            Some(db_score) => db_score.try_into(),
            None => Err(anyhow::anyhow!(
                "No scores for guild {guild_id} found in the `guild_scores` table"
            )),
        }
    }
}