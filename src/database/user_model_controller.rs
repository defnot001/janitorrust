use chrono::{DateTime, Utc};
use serenity::all::{GuildId, UserId};
use sqlx::{prelude::FromRow, PgPool};

#[derive(Debug)]
pub enum UserType {
    Reporter,
    Listener,
}

#[derive(Debug, FromRow)]
struct DbUser {
    id: String,
    user_type: String,
    servers: Vec<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct User {
    pub id: UserId,
    pub user_type: UserType,
    pub servers: Vec<GuildId>,
}

impl TryFrom<DbUser> for User {
    type Error = anyhow::Error;

    fn try_from(db_user: DbUser) -> Result<Self, Self::Error> {
        let id = UserId::from(db_user.id.parse::<u64>()?);
        let user_type = deserialize_user_type(&db_user.user_type)?;

        let mut servers: Vec<GuildId> = Vec::new();

        for server_id in db_user.servers {
            servers.push(GuildId::from(server_id.parse::<u64>()?));
        }

        Ok(User {
            id,
            user_type,
            servers,
        })
    }
}

pub struct UserModelController;

impl UserModelController {
    pub async fn get(db_pool: &PgPool, user_id: UserId) -> anyhow::Result<Option<User>> {
        let db_user = sqlx::query_as::<_, DbUser>("SELECT * FROM users WHERE id = $1;")
            .bind(user_id.to_string())
            .fetch_optional(db_pool)
            .await?;

        match db_user {
            Some(db_user) => Ok(Some(User::try_from(db_user)?)),
            None => Ok(None),
        }
    }

    pub async fn create(
        db_pool: &PgPool,
        user_id: UserId,
        user_type: UserType,
        servers: Vec<GuildId>,
    ) -> anyhow::Result<User> {
        let servers = servers
            .iter()
            .map(|server_id| server_id.to_string())
            .collect::<Vec<String>>();

        sqlx::query_as::<_, DbUser>(
            "INSERT INTO users (id, user_type, servers) VALUES ($1, $2, $3) RETURNING *;",
        )
        .bind(user_id.to_string())
        .bind(serialize_user_type(&user_type))
        .bind(servers)
        .fetch_one(db_pool)
        .await?
        .try_into()
    }

    pub async fn update(
        db_pool: &PgPool,
        user_id: UserId,
        user_type: UserType,
        servers: Vec<GuildId>,
    ) -> anyhow::Result<User> {
        let servers = servers
            .iter()
            .map(|server_id| server_id.to_string())
            .collect::<Vec<String>>();

        sqlx::query_as::<_, DbUser>(
            "UPDATE users SET user_type = $1, servers = $2 WHERE id = $3 RETURNING *;",
        )
        .bind(serialize_user_type(&user_type))
        .bind(servers)
        .bind(user_id.to_string())
        .fetch_one(db_pool)
        .await?
        .try_into()
    }

    pub async fn delete(db_pool: &PgPool, user_id: UserId) -> anyhow::Result<User> {
        sqlx::query_as::<_, DbUser>("DELETE FROM users WHERE id = $1 RETURNING *;")
            .bind(user_id.to_string())
            .fetch_one(db_pool)
            .await?
            .try_into()
    }

    pub async fn get_all(db_pool: &PgPool, limit: u8) -> anyhow::Result<Vec<User>> {
        let db_users = sqlx::query_as::<_, DbUser>("SELECT * FROM users LIMIT $1;")
            .bind(limit as i16)
            .fetch_all(db_pool)
            .await?;

        let mut users: Vec<User> = Vec::new();

        for db_user in db_users {
            users.push(User::try_from(db_user)?);
        }

        Ok(users)
    }

    pub async fn get_by_guild(db_pool: &PgPool, guild_id: &GuildId) -> anyhow::Result<Vec<User>> {
        let db_users =
            sqlx::query_as::<_, DbUser>("SELECT * FROM users WHERE $1 = ANY(servers) LIMIT $2;")
                .bind(guild_id.to_string())
                .bind(10 as i16)
                .fetch_all(db_pool)
                .await?;

        let mut users: Vec<User> = Vec::new();

        for db_user in db_users {
            users.push(User::try_from(db_user)?);
        }

        Ok(users)
    }
}

fn deserialize_user_type(user_type: &str) -> anyhow::Result<UserType> {
    match user_type {
        "reporter" => Ok(UserType::Reporter),
        "listener" => Ok(UserType::Listener),
        _ => Err(anyhow::anyhow!("Invalid user type")),
    }
}

fn serialize_user_type(user_type: &UserType) -> &'static str {
    match user_type {
        UserType::Reporter => "reporter",
        UserType::Listener => "listener",
    }
}
