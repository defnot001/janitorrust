use std::str::FromStr;

use poise::serenity_prelude as serenity;
use serenity::{GuildId, UserId};
use sqlx::{prelude::FromRow, PgPool};

#[derive(Debug, FromRow)]
struct DbUserScoreboard {
    discord_id: String,
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
        let user_id = UserId::from_str(&db_user_scoreboard.discord_id)?;
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
        let mut tx = db_pool.begin().await?;

        let user_res = sqlx::query(
            r#"
            INSERT INTO user_scores (discord_id, score)
            VALUES ($1, 1)
            ON CONFLICT (discord_id)
            DO UPDATE SET score = user_scores.score + 1;
            "#,
        )
        .bind(user_id.to_string())
        .execute(&mut *tx)
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
        .execute(&mut *tx)
        .await;

        if user_res.is_err() || guild_res.is_err() {
            tx.rollback().await?;
            return Err(anyhow::anyhow!("Failed to create or increase scoreboards"));
        }

        tx.commit().await?;

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
        .await?;

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
        .await?;

        db_top_guilds
            .into_iter()
            .map(GuildScoreboard::try_from)
            .collect::<Result<Vec<GuildScoreboard>, _>>()
    }

    pub async fn get_user_score(
        db_pool: &PgPool,
        user_id: UserId,
    ) -> anyhow::Result<UserScoreboard> {
        let db_score = sqlx::query_as::<_, DbUserScoreboard>(
            "SELECT * FROM user_scores WHERE discord_id = $1;",
        )
        .bind(user_id.to_string())
        .fetch_optional(db_pool)
        .await?;

        match db_score {
            Some(db_score) => db_score.try_into(),
            None => Ok(UserScoreboard { score: 0, user_id }),
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
        .await?;

        match db_score {
            Some(db_score) => db_score.try_into(),
            None => Ok(GuildScoreboard { score: 0, guild_id }),
        }
    }
}
