use std::str::FromStr;

use anyhow::Context;
use chrono::{DateTime, Utc};
use poise::serenity_prelude as serenity;
use serenity::{User, UserId};
use sqlx::{FromRow, PgPool};

use crate::AppContext;

#[derive(Debug, FromRow, Clone)]
struct DbAdmin {
    user_id: String,
    created_at: DateTime<Utc>,
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
            user_id: UserId::from_str(&db_admin.user_id)?,
            created_at: db_admin.created_at,
        })
    }
}

pub struct AdminModelController;

impl AdminModelController {
    pub async fn get_all(db_pool: &PgPool) -> anyhow::Result<Vec<Admin>> {
        let db_admins = sqlx::query_as::<_, DbAdmin>("SELECT * FROM admins;")
            .fetch_all(db_pool)
            .await
            .context("Failed to get all admins from the `admins` table")?;

        db_admins
            .into_iter()
            .map(Admin::try_from)
            .collect::<Result<Vec<Admin>, _>>()
    }

    pub async fn get(db_pool: &PgPool, id: &UserId) -> anyhow::Result<Option<Admin>> {
        let db_admin = sqlx::query_as::<_, DbAdmin>("SELECT * FROM admins WHERE user_id = $1;")
            .bind(id.to_string())
            .fetch_optional(db_pool)
            .await
            .context(format!(
                "Failed to get admin with id {id} from the `admins` table"
            ))?;

        db_admin.map(Admin::try_from).transpose()
    }
}
