use std::str::FromStr;

use chrono::NaiveDateTime;
use chrono::{DateTime, Utc};
use poise::serenity_prelude as serenity;
use serenity::{CreateEmbed, GuildId, PartialGuild, User as SerenityUser, UserId};
use sqlx::{prelude::FromRow, PgPool};

use crate::util::embeds;
use crate::util::format;

#[derive(Debug, poise::ChoiceParameter)]
pub enum UserType {
    Reporter,
    Listener,
}

impl std::str::FromStr for UserType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "reporter" => Ok(Self::Reporter),
            "listener" => Ok(Self::Listener),
            _ => anyhow::bail!("Unknown UserType: {s}"),
        }
    }
}

impl std::fmt::Display for UserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reporter => write!(f, "reporter"),
            Self::Listener => write!(f, "listener"),
        }
    }
}

#[derive(Debug, FromRow)]
struct DbUser {
    id: String,
    user_type: String,
    servers: Vec<String>,
    created_at: NaiveDateTime,
}

#[derive(Debug)]
pub struct JanitorUser {
    pub user_id: UserId,
    pub user_type: UserType,
    pub guild_ids: Vec<GuildId>,
    pub created_at: DateTime<Utc>,
}

impl JanitorUser {
    pub fn to_embed(
        &self,
        interaction_user: &SerenityUser,
        target_user: &SerenityUser,
        guilds: &[PartialGuild],
    ) -> CreateEmbed {
        let guilds = guilds.iter().map(format::fdisplay).collect::<Vec<_>>();

        embeds::CreateJanitorEmbed::new(interaction_user)
            .into_embed()
            .title(format!("User Info {}", format::fdisplay(target_user)))
            .field("Servers", guilds.join("\n"), false)
            .field("Created At", format::display_time(self.created_at), false)
            .field("User Type", self.user_type.to_string(), false)
    }
}

pub struct CreateJanitorUser<'a> {
    pub user_id: UserId,
    pub user_type: UserType,
    pub guild_ids: &'a [GuildId],
}

impl TryFrom<DbUser> for JanitorUser {
    type Error = anyhow::Error;

    fn try_from(db_user: DbUser) -> Result<Self, Self::Error> {
        let user_id = UserId::from_str(&db_user.id)?;
        let user_type = UserType::from_str(&db_user.user_type)?;
        let guild_ids = db_user
            .servers
            .into_iter()
            .map(|g| GuildId::from_str(&g).map_err(anyhow::Error::from))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(JanitorUser {
            user_id,
            user_type,
            guild_ids,
            created_at: db_user.created_at.and_utc(),
        })
    }
}

pub struct UserModelController;

impl UserModelController {
    pub async fn get(db_pool: &PgPool, user_id: UserId) -> anyhow::Result<Option<JanitorUser>> {
        let db_user = sqlx::query_as::<_, DbUser>("SELECT * FROM users WHERE id = $1;")
            .bind(user_id.to_string())
            .fetch_optional(db_pool)
            .await?;

        db_user.map(JanitorUser::try_from).transpose()
    }

    pub async fn create<'a>(
        db_pool: &PgPool,
        user: CreateJanitorUser<'a>,
    ) -> anyhow::Result<JanitorUser> {
        let CreateJanitorUser {
            user_id,
            user_type,
            guild_ids,
        } = user;

        let guild_ids = guild_ids
            .iter()
            .map(|server_id| server_id.to_string())
            .collect::<Vec<String>>();

        let db_user = sqlx::query_as::<_, DbUser>(
            "INSERT INTO users (id, user_type, servers) VALUES ($1, $2, $3) RETURNING *;",
        )
        .bind(user_id.to_string())
        .bind(user_type.to_string())
        .bind(guild_ids)
        .fetch_one(db_pool)
        .await;

        let db_user = match db_user {
            Ok(user) => user,
            Err(e) => {
                let Some(db_error) = e.as_database_error() else {
                    return Err(anyhow::Error::from(e));
                };

                if db_error.is_unique_violation() {
                    anyhow::bail!("Unique key violation")
                }

                return Err(anyhow::Error::from(e));
            }
        };

        JanitorUser::try_from(db_user)
    }

    pub async fn update<'a>(
        db_pool: &PgPool,
        user: CreateJanitorUser<'a>,
    ) -> anyhow::Result<JanitorUser> {
        let CreateJanitorUser {
            user_id,
            user_type,
            guild_ids,
        } = user;

        let guild_ids = guild_ids
            .iter()
            .map(|server_id| server_id.to_string())
            .collect::<Vec<String>>();

        sqlx::query_as::<_, DbUser>(
            "UPDATE users SET user_type = $2, servers = $3 WHERE id = $1 RETURNING *;",
        )
        .bind(user_id.to_string())
        .bind(user_type.to_string())
        .bind(guild_ids)
        .fetch_one(db_pool)
        .await?
        .try_into()
    }

    pub async fn delete(db_pool: &PgPool, user_id: UserId) -> anyhow::Result<JanitorUser> {
        sqlx::query_as::<_, DbUser>("DELETE FROM users WHERE id = $1 RETURNING *;")
            .bind(user_id.to_string())
            .fetch_one(db_pool)
            .await?
            .try_into()
    }

    pub async fn get_by_guild(
        db_pool: &PgPool,
        guild_id: GuildId,
    ) -> anyhow::Result<Vec<JanitorUser>> {
        let db_users =
            sqlx::query_as::<_, DbUser>("SELECT * FROM users WHERE $1 = ANY(servers) LIMIT 10;")
                .bind(guild_id.to_string())
                .fetch_all(db_pool)
                .await?;

        db_users
            .into_iter()
            .map(JanitorUser::try_from)
            .collect::<anyhow::Result<Vec<_>>>()
    }
}
