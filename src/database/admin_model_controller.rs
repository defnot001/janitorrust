use std::str::FromStr;

use chrono::{DateTime, Utc};
use poise::serenity_prelude as serenity;
use serenity::UserId;
use sqlx::{FromRow, PgPool};

#[derive(Debug, FromRow)]
struct DbAdmin {
    id: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct Admin {
    pub id: UserId,
    pub created_at: DateTime<Utc>,
}

impl TryFrom<DbAdmin> for Admin {
    type Error = anyhow::Error;

    fn try_from(db_admin: DbAdmin) -> Result<Self, Self::Error> {
        let user_id = UserId::from_str(db_admin.id.as_str())?;

        Ok(Admin {
            id: user_id,
            created_at: db_admin.created_at,
        })
    }
}

pub struct AdminModelController;

impl AdminModelController {
    pub async fn get_all(db_pool: &PgPool) -> anyhow::Result<Vec<Admin>> {
        sqlx::query_as::<_, DbAdmin>("SELECT * FROM admins;")
            .fetch_all(db_pool)
            .await?
            .into_iter()
            .map(Admin::try_from)
            .collect::<Result<Vec<Admin>, _>>()
    }

    pub async fn get(db_pool: &PgPool, id: &UserId) -> anyhow::Result<Option<Admin>> {
        sqlx::query_as::<_, DbAdmin>("SELECT * FROM admins WHERE id = ?;")
            .bind(id.to_string())
            .fetch_optional(db_pool)
            .await?
            .map(Admin::try_from)
            .transpose()
    }

    pub async fn is_admin(db_pool: &PgPool, id: &UserId) -> bool {
        sqlx::query_as::<_, DbAdmin>("SELECT * FROM admins WHERE id = ?;")
            .bind(id.to_string())
            .fetch_optional(db_pool)
            .await
            .map(|db_admin| db_admin.is_some())
            .unwrap_or(false)
    }
}
