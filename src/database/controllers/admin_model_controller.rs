use std::str::FromStr;

use anyhow::Context;
use chrono::{DateTime, NaiveDateTime, Utc};
use poise::serenity_prelude as serenity;
use serenity::{User, UserId};
use sqlx::{FromRow, PgPool};

use crate::AppContext;

#[derive(Debug, FromRow, Clone)]
struct DbAdmin {
    id: String,
    created_at: NaiveDateTime,
}

#[derive(Debug)]
pub struct Admin {
    pub user_id: UserId,
    pub created_at: DateTime<Utc>,
}

impl Admin {
    pub async fn into_user(self, ctx: AppContext<'_>) -> anyhow::Result<User> {
        self.user_id.to_user(ctx).await.context(format!(
            "Failed to get user with id {} from the API",
            self.user_id
        ))
    }
}

impl TryFrom<DbAdmin> for Admin {
    type Error = anyhow::Error;

    fn try_from(db_admin: DbAdmin) -> Result<Self, Self::Error> {
        Ok(Admin {
            user_id: UserId::from_str(&db_admin.id)?,
            created_at: db_admin.created_at.and_utc(),
        })
    }
}

pub struct AdminModelController;

impl AdminModelController {
    pub async fn get_all(db_pool: &PgPool) -> anyhow::Result<Vec<Admin>> {
        let db_admins = sqlx::query_as::<_, DbAdmin>("SELECT * FROM admins;")
            .fetch_all(db_pool)
            .await?;

        db_admins
            .into_iter()
            .map(Admin::try_from)
            .collect::<Result<Vec<Admin>, _>>()
    }

    pub async fn get(db_pool: &PgPool, id: &UserId) -> anyhow::Result<Option<Admin>> {
        let db_admin = sqlx::query_as::<_, DbAdmin>("SELECT * FROM admins WHERE id = $1;")
            .bind(id.to_string())
            .fetch_optional(db_pool)
            .await?;

        db_admin.map(Admin::try_from).transpose()
    }
}
