use serenity::all::{GuildId, UserId};
use sqlx::{prelude::FromRow, PgPool};

#[derive(Debug, FromRow)]
struct DbUserScoreboard {
    discord_id: String,
    score: i32,
}

#[derive(Debug, FromRow)]
struct DbGuildScoreboard {
    discord_id: String,
    score: i32,
}

#[derive(Debug)]
pub struct UserScoreboard {
    pub discord_id: UserId,
    pub score: u32,
}

#[derive(Debug)]
pub struct GuildScoreboard {
    pub discord_id: UserId,
    pub score: u32,
}

#[derive(Debug)]
pub struct Scoreboard {
    pub user_scoreboards: UserScoreboard,
    pub guild_scoreboards: GuildScoreboard,
}

impl TryFrom<DbUserScoreboard> for UserScoreboard {
    type Error = anyhow::Error;

    fn try_from(db_user_scoreboard: DbUserScoreboard) -> Result<Self, Self::Error> {
        Ok(UserScoreboard {
            discord_id: UserId::from(db_user_scoreboard.discord_id.parse::<u64>()?),
            score: db_user_scoreboard.score as u32,
        })
    }
}

impl TryFrom<DbGuildScoreboard> for GuildScoreboard {
    type Error = anyhow::Error;

    fn try_from(db_guild_scoreboard: DbGuildScoreboard) -> Result<Self, Self::Error> {
        Ok(GuildScoreboard {
            discord_id: UserId::from(db_guild_scoreboard.discord_id.parse::<u64>()?),
            score: db_guild_scoreboard.score as u32,
        })
    }
}

pub struct ScoresModelController;

impl ScoresModelController {
    pub async fn create_or_increase_scoreboards(
        db_pool: &PgPool,
        user_id: UserId,
        guild_id: GuildId,
    ) -> anyhow::Result<Scoreboard> {
        sqlx::query("BEGIN").execute(db_pool).await?;

        let user_scoreboards = sqlx::query_as::<_, DbUserScoreboard>(
            r#"
            INSERT INTO user_scores (discord_id, score)
            VALUES ($1, 1)
            ON CONFLICT (discord_id)
            DO UPDATE SET score = user_scores.score + 1
            RETURNING *;
            "#,
        )
        .fetch_one(db_pool)
        .await;

        let guild_scoreboards = sqlx::query_as::<_, DbGuildScoreboard>(
            r#"
            INSERT INTO guild_scores (guild_id, score)
            VALUES ($1, 1)
            ON CONFLICT (guild_id)
            DO UPDATE SET score = guild_scores.score + 1
            RETURNING *;
            "#,
        )
        .fetch_one(db_pool)
        .await;

        if user_scoreboards.is_err() || guild_scoreboards.is_err() {
            sqlx::query("ROLLBACK").execute(db_pool).await?;
            return Err(anyhow::anyhow!("Failed to create or increase scoreboards"));
        }

        sqlx::query("COMMIT").execute(db_pool).await?;

        Ok(Scoreboard {
            user_scoreboards: UserScoreboard::try_from(user_scoreboards.unwrap())?,
            guild_scoreboards: GuildScoreboard::try_from(guild_scoreboards.unwrap())?,
        })
    }

    pub async fn get_top_users(db_pool: &PgPool, limit: u8) -> anyhow::Result<Vec<UserScoreboard>> {
        sqlx::query_as::<_, DbUserScoreboard>(
            r#"
            SELECT * FROM user_scores
            ORDER BY score DESC
            LIMIT $1;
            "#,
        )
        .bind(limit as i16)
        .fetch_all(db_pool)
        .await?
        .into_iter()
        .map(UserScoreboard::try_from)
        .collect::<Result<Vec<UserScoreboard>, _>>()
    }

    pub async fn get_top_guilds(
        db_pool: &PgPool,
        limit: u8,
    ) -> anyhow::Result<Vec<GuildScoreboard>> {
        sqlx::query_as::<_, DbGuildScoreboard>(
            r#"
            SELECT * FROM guild_scores
            ORDER BY score DESC
            LIMIT $1;
            "#,
        )
        .bind(limit as i16)
        .fetch_all(db_pool)
        .await?
        .into_iter()
        .map(GuildScoreboard::try_from)
        .collect::<Result<Vec<GuildScoreboard>, _>>()
    }

    pub async fn get_user_score(
        db_pool: &PgPool,
        user_id: UserId,
    ) -> anyhow::Result<Option<UserScoreboard>> {
        sqlx::query_as::<_, DbUserScoreboard>("SELECT * FROM user_scores WHERE discord_id = ?;")
            .bind(user_id.to_string())
            .fetch_optional(db_pool)
            .await?
            .map(UserScoreboard::try_from)
            .transpose()
    }

    pub async fn get_guild_score(
        db_pool: &PgPool,
        guild_id: GuildId,
    ) -> anyhow::Result<Option<GuildScoreboard>> {
        sqlx::query_as::<_, DbGuildScoreboard>("SELECT * FROM guild_scores WHERE discord_id = ?;")
            .bind(guild_id.to_string())
            .fetch_optional(db_pool)
            .await?
            .map(GuildScoreboard::try_from)
            .transpose()
    }
}
