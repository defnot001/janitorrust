use chrono::{DateTime, Utc};
use serenity::all::{ChannelId, GuildId, RoleId, UserId};
use sqlx::{prelude::FromRow, PgPool};

#[derive(Debug)]
pub enum ActionLevel {
    Notify,
    Timeout,
    Kick,
    SoftBan,
    Ban,
}

fn stringify_action_level(level: ActionLevel) -> String {
    match level {
        ActionLevel::Notify => "Notify".to_string(),
        ActionLevel::Timeout => "Timeout".to_string(),
        ActionLevel::Kick => "Kick".to_string(),
        ActionLevel::SoftBan => "Soft Ban".to_string(),
        ActionLevel::Ban => "Ban".to_string(),
    }
}

fn decode_action_level(level: i8) -> ActionLevel {
    match level {
        0 => ActionLevel::Notify,
        1 => ActionLevel::Timeout,
        2 => ActionLevel::Kick,
        3 => ActionLevel::SoftBan,
        4 => ActionLevel::Ban,
        _ => ActionLevel::Notify,
    }
}

#[derive(Debug, FromRow)]
struct DbServerConfig {
    server_id: String,
    log_channel: Option<String>,
    ping_users: bool,
    ping_role: Option<String>,
    spam_action_level: i8,
    impersonation_action_level: i8,
    bigotry_action_level: i8,
    timeout_users_with_role: bool,
    ignored_roles: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct ServerConfig {
    pub server_id: GuildId,
    pub log_channel: Option<ChannelId>,
    pub ping_users: bool,
    pub ping_role: Option<RoleId>,
    pub spam_action_level: ActionLevel,
    pub impersonation_action_level: ActionLevel,
    pub bigotry_action_level: ActionLevel,
    pub timeout_users_with_role: bool,
    pub ignored_roles: Vec<RoleId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct ServerConfigWithUserIds {
    pub server_config: ServerConfig,
    pub user_ids: Vec<UserId>,
}

pub struct UpdateServerConfig {
    pub log_channel: Option<ChannelId>,
    pub ping_users: Option<bool>,
    pub ping_role: Option<RoleId>,
    pub spam_action_level: Option<ActionLevel>,
    pub impersonation_action_level: Option<ActionLevel>,
    pub bigotry_action_level: Option<ActionLevel>,
    pub timeout_users_with_role: Option<bool>,
    pub ignored_roles: Option<Vec<RoleId>>,
}

#[derive(Debug, FromRow)]
struct ServerIdQuery {
    server_id: String,
}

impl TryFrom<DbServerConfig> for ServerConfig {
    type Error = anyhow::Error;

    fn try_from(value: DbServerConfig) -> Result<Self, Self::Error> {
        let server_id = GuildId::from(value.server_id.parse::<u64>()?);

        let log_channel = if let Some(channel_id) = value.log_channel {
            Some(ChannelId::from(channel_id.parse::<u64>()?))
        } else {
            None
        };

        let ping_role = if let Some(role_id) = value.ping_role {
            Some(RoleId::from(role_id.parse::<u64>()?))
        } else {
            None
        };

        let mut ignored_roles: Vec<RoleId> = Vec::new();

        for role_id in value.ignored_roles.iter() {
            ignored_roles.push(RoleId::from(role_id.parse::<u64>()?));
        }

        let spam_action_level = decode_action_level(value.spam_action_level);
        let impersonation_action_level = decode_action_level(value.impersonation_action_level);
        let bigotry_action_level = decode_action_level(value.bigotry_action_level);

        Ok(ServerConfig {
            server_id,
            log_channel,
            ping_users: value.ping_users,
            ping_role,
            spam_action_level,
            impersonation_action_level,
            bigotry_action_level,
            timeout_users_with_role: value.timeout_users_with_role,
            ignored_roles,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

pub struct ServerConfigModelController;

impl ServerConfigModelController {
    pub async fn create_default_if_not_exists(
        pg_pool: &PgPool,
        guild_id: GuildId,
    ) -> anyhow::Result<ServerConfig> {
        sqlx::query_as::<_, DbServerConfig>(
            r#"
            INSERT INTO server_configs (server_id) 
            VALUES ($1) 
            ON CONFLICT (server_id) DO NOTHING 
            RETURNING *;
            "#,
        )
        .bind(guild_id.to_string())
        .fetch_one(pg_pool)
        .await
        .map(ServerConfig::try_from)?
    }

    pub async fn get_by_guild_id(
        pg_pool: &PgPool,
        guild_id: GuildId,
    ) -> anyhow::Result<ServerConfig> {
        sqlx::query_as::<_, DbServerConfig>("SELECT * FROM server_configs WHERE server_id = $1;")
            .bind(guild_id.to_string())
            .fetch_one(pg_pool)
            .await
            .map(ServerConfig::try_from)?
    }

    pub async fn get_multiple_by_guild_id(
        pg_pool: &PgPool,
        guild_ids: Vec<GuildId>,
    ) -> anyhow::Result<Vec<ServerConfig>> {
        let guild_ids: Vec<String> = guild_ids.iter().map(|id| id.to_string()).collect();

        sqlx::query_as::<_, DbServerConfig>(
            "SELECT * FROM server_configs WHERE server_id = ANY($1::text[]);",
        )
        .bind(&guild_ids)
        .fetch_all(pg_pool)
        .await?
        .into_iter()
        .map(ServerConfig::try_from)
        .collect()
    }

    pub async fn get_all(pg_pool: &PgPool) -> anyhow::Result<Vec<ServerConfig>> {
        sqlx::query_as::<_, DbServerConfig>("SELECT * FROM server_configs;")
            .fetch_all(pg_pool)
            .await?
            .into_iter()
            .map(ServerConfig::try_from)
            .collect()
    }

    pub async fn get_all_guild_ids(pg_pool: &PgPool) -> anyhow::Result<Vec<GuildId>> {
        let server_ids =
            sqlx::query_as::<_, ServerIdQuery>("SELECT server_id FROM server_configs;")
                .fetch_all(pg_pool)
                .await?
                .into_iter()
                .map(|server_id| server_id.server_id)
                .collect::<Vec<String>>();

        let mut guild_ids: Vec<GuildId> = Vec::new();

        for server_id in server_ids {
            let guild_id = GuildId::from(server_id.parse::<u64>()?);
            guild_ids.push(guild_id);
        }

        Ok(guild_ids)
    }

    pub async fn update(
        pg_pool: &PgPool,
        guild_id: GuildId,
        update: UpdateServerConfig,
    ) -> anyhow::Result<ServerConfig> {
        let previous = sqlx::query_as::<_, DbServerConfig>(
            "SELECT * FROM server_configs WHERE server_id = $1;",
        )
        .bind(guild_id.to_string())
        .fetch_optional(pg_pool)
        .await?;

        if previous.is_none() {
            return Err(anyhow::anyhow!("Server config does not exist"));
        }

        let previous = previous.unwrap();

        let log_channel = if let Some(channel_id) = update.log_channel {
            Some(channel_id.to_string())
        } else {
            previous.log_channel
        };

        let ping_users = update.ping_users.unwrap_or(previous.ping_users);

        let ping_role = if let Some(role_id) = update.ping_role {
            Some(role_id.to_string())
        } else {
            previous.ping_role
        };

        let spam_action_level = update
            .spam_action_level
            .map(|level| level as i8)
            .unwrap_or(previous.spam_action_level);

        let impersonation_action_level = update
            .impersonation_action_level
            .map(|level| level as i8)
            .unwrap_or(previous.impersonation_action_level);

        let bigotry_action_level = update
            .bigotry_action_level
            .map(|level| level as i8)
            .unwrap_or(previous.bigotry_action_level);

        let timeout_users_with_role = update
            .timeout_users_with_role
            .unwrap_or(previous.timeout_users_with_role);

        let ignored_roles = if let Some(ignored_roles) = update.ignored_roles {
            ignored_roles
                .iter()
                .map(|role_id| role_id.to_string())
                .collect()
        } else {
            previous.ignored_roles
        };

        sqlx::query_as::<_, DbServerConfig>(
            r#"
            UPDATE server_configs 
            SET log_channel = $2, 
                ping_users = $3, 
                ping_role = $4, 
                spam_action_level = $5, 
                impersonation_action_level = $6, 
                bigotry_action_level = $7, 
                timeout_users_with_role = $8, 
                ignored_roles = $9, 
                updated_at = now() 
            WHERE server_id = $1 
            RETURNING *;
            "#,
        )
        .bind(guild_id.to_string())
        .bind(log_channel)
        .bind(ping_users)
        .bind(ping_role)
        .bind(spam_action_level)
        .bind(impersonation_action_level)
        .bind(bigotry_action_level)
        .bind(timeout_users_with_role)
        .bind(&ignored_roles)
        .fetch_one(pg_pool)
        .await
        .map(ServerConfig::try_from)?
    }

    pub async fn delete(pg_pool: &PgPool, guild_id: GuildId) -> anyhow::Result<ServerConfig> {
        let server_config = sqlx::query_as::<_, DbServerConfig>(
            "DELETE FROM server_configs WHERE server_id = $1 RETURNING *;",
        )
        .bind(guild_id.to_string())
        .fetch_one(pg_pool)
        .await?;

        ServerConfig::try_from(server_config)
    }

    pub async fn delete_if_needed() {
        todo!()
    }
}
