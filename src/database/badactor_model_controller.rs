use chrono::{DateTime, Utc};
use serenity::all::{GuildId, UserId};
use sqlx::{FromRow, PgPool};

#[derive(Debug, FromRow)]
struct DbBadActor {
    id: i64,
    user_id: String,
    is_active: bool,
    actor_type: String,
    screenshot_proof: Option<String>,
    explanation: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    originally_created_in: String,
    last_changed_by: String,
}

#[derive(Debug)]
pub enum BadActorType {
    Spam,
    Impersonation,
    Bigotry,
}

#[derive(Debug)]
pub struct BadActor {
    pub id: u64,
    pub user_id: UserId,
    pub is_active: bool,
    pub actor_type: BadActorType,
    pub screenshot_proof: Option<String>,
    pub explanation: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub originally_created_in: GuildId,
    pub last_changed_by: UserId,
}

impl TryFrom<DbBadActor> for BadActor {
    type Error = anyhow::Error;

    fn try_from(db_bad_actor: DbBadActor) -> Result<Self, Self::Error> {
        let actor_type = match db_bad_actor.actor_type.as_str() {
            "spam" => BadActorType::Spam,
            "impersonation" => BadActorType::Impersonation,
            "bigotry" => BadActorType::Bigotry,
            _ => return Err(anyhow::anyhow!("Invalid actor type")),
        };

        Ok(BadActor {
            id: db_bad_actor.id as u64,
            user_id: UserId::from(db_bad_actor.user_id.parse::<u64>()?),
            is_active: db_bad_actor.is_active,
            actor_type,
            screenshot_proof: db_bad_actor.screenshot_proof,
            explanation: db_bad_actor.explanation,
            created_at: db_bad_actor.created_at,
            updated_at: db_bad_actor.updated_at,
            originally_created_in: GuildId::from(
                db_bad_actor.originally_created_in.parse::<u64>()?,
            ),
            last_changed_by: UserId::from(db_bad_actor.last_changed_by.parse::<u64>()?),
        })
    }
}

pub struct CreateBadActorOptions {
    pub user_id: UserId,
    pub actor_type: BadActorType,
    pub screenshot_proof: Option<String>,
    pub explanation: Option<String>,
    pub originally_created_in: GuildId,
    pub last_changed_by: UserId,
}

pub enum BadActorQueryType {
    All,
    Active,
    Inactive,
}

struct BadActorModelController;

impl BadActorModelController {
    pub async fn create(
        db_pool: &PgPool,
        options: CreateBadActorOptions,
    ) -> anyhow::Result<BadActor> {
        let actor_type = match options.actor_type {
            BadActorType::Spam => "spam",
            BadActorType::Impersonation => "impersonation",
            BadActorType::Bigotry => "bigotry",
        };

        sqlx::query_as::<_, DbBadActor>(
            r#"
            INSERT INTO badactors (user_id, is_active, actor_type, screenshot_proof, explanation, originally_created_in, last_changed_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *;
            "#,
        )
        .bind(options.user_id.to_string())
        .bind(true)
        .bind(actor_type)
        .bind(options.screenshot_proof)
        .bind(options.explanation)
        .bind(options.originally_created_in.to_string())
        .bind(options.last_changed_by.to_string())
        .fetch_one(db_pool)
        .await?
        .try_into()
    }

    pub async fn get_by_user_id(
        db_pool: &PgPool,
        user_id: UserId,
    ) -> anyhow::Result<Vec<BadActor>> {
        sqlx::query_as::<_, DbBadActor>("SELECT * FROM badactors WHERE user_id = $1;")
            .bind(user_id.to_string())
            .fetch_all(db_pool)
            .await?
            .into_iter()
            .map(|db_bad_actor| BadActor::try_from(db_bad_actor))
            .collect::<Result<Vec<BadActor>, _>>()
    }

    pub async fn get_by_id(db_pool: &PgPool, id: u64) -> anyhow::Result<Option<BadActor>> {
        sqlx::query_as::<_, DbBadActor>("SELECT * FROM badactors WHERE id = $1;")
            .bind(id as i64)
            .fetch_optional(db_pool)
            .await?
            .map(|db_bad_actor| BadActor::try_from(db_bad_actor))
            .transpose()
    }

    pub async fn deavtivate(
        db_pool: &PgPool,
        id: u64,
        explanation: impl Into<String>,
        last_changed_by: UserId,
    ) -> anyhow::Result<BadActor> {
        sqlx::query_as::<_, DbBadActor>(
            r#"
            UPDATE bad_actors 
            SET 
                is_active = false, 
                explanation = $2, 
                last_changed_by = $3, 
                updated_at = CURRENT_TIMESTAMP 
            WHERE id = $1 
            RETURNING *;
            "#,
        )
        .bind(id as i64)
        .bind(explanation.into())
        .bind(last_changed_by.to_string())
        .fetch_one(db_pool)
        .await?
        .try_into()
    }

    pub async fn activate(
        db_pool: &PgPool,
        id: u64,
        explanation: impl Into<String>,
        last_changed_by: UserId,
    ) -> anyhow::Result<BadActor> {
        sqlx::query_as::<_, DbBadActor>(
            r#"
            UPDATE bad_actors 
            SET 
                is_active = true, 
                explanation = $2, 
                last_changed_by = $3, 
                updated_at = CURRENT_TIMESTAMP 
            WHERE id = $1 
            RETURNING *;
            "#,
        )
        .bind(id as i64)
        .bind(explanation.into())
        .bind(last_changed_by.to_string())
        .fetch_one(db_pool)
        .await?
        .try_into()
    }

    pub async fn get(
        db_pool: &PgPool,
        limit: u8,
        query_type: Option<BadActorQueryType>,
    ) -> anyhow::Result<Vec<BadActor>> {
        let query_type = query_type.unwrap_or(BadActorQueryType::All);

        let mut query_str = String::new();

        match query_type {
            BadActorQueryType::All => {
                query_str =
                    "SELECT * FROM bad_actors ORDER BY created_at DESC LIMIT $1;".to_string()
            }
            BadActorQueryType::Active => query_str =
                "SELECT * FROM bad_actors WHERE is_active = true ORDER BY created_at DESC LIMIT $1"
                    .to_string(),
            BadActorQueryType::Inactive => query_str =
                "SELECT * FROM bad_actors WHERE is_active = false ORDER BY created_at DESC LIMIT $1"
                    .to_string(),
        };

        sqlx::query_as::<_, DbBadActor>(&query_str)
            .bind(limit as i8)
            .fetch_all(db_pool)
            .await?
            .into_iter()
            .map(|db_bad_actor| BadActor::try_from(db_bad_actor))
            .collect::<Result<Vec<BadActor>, _>>()
    }
}
