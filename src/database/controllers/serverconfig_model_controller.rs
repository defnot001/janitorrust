use std::str::FromStr;

use anyhow::Context;
use chrono::{DateTime, Utc};
use futures::TryFutureExt;
use poise::serenity_prelude as serenity;
use serenity::{
    ChannelId, CreateEmbed, GuildId, Mentionable, PartialGuild, RoleId, User as SerenityUser,
    UserId,
};
use sqlx::{prelude::FromRow, PgPool};

use crate::database::controllers::user_model_controller::UserModelController;
use crate::util::{embeds, format};
use crate::Context as AppContext;

#[derive(Debug, Clone, Copy, PartialEq, poise::ChoiceParameter)]
#[repr(i8)]
pub enum ActionLevel {
    Notify,
    Timeout,
    Kick,
    SoftBan,
    Ban,
}

impl std::fmt::Display for ActionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Notify => write!(f, "notify"),
            Self::Timeout => write!(f, "timeout"),
            Self::Kick => write!(f, "kick"),
            Self::SoftBan => write!(f, "softban"),
            Self::Ban => write!(f, "ban"),
        }
    }
}

impl TryFrom<i8> for ActionLevel {
    type Error = anyhow::Error;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Notify),
            1 => Ok(Self::Timeout),
            2 => Ok(Self::Kick),
            3 => Ok(Self::SoftBan),
            4 => Ok(Self::Ban),
            _ => {
                anyhow::bail!("Unknown action level: {value}")
            }
        }
    }
}

#[derive(Debug, FromRow)]
struct DbServerConfig {
    guild_id: String,
    log_channel_id: Option<String>,
    ping_users: bool,
    ping_role: Option<String>,
    spam_action_level: i8,
    impersonation_action_level: i8,
    bigotry_action_level: i8,
    ignored_roles: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub guild_id: GuildId,
    pub log_channel_id: Option<ChannelId>,
    pub ping_users: bool,
    pub ping_role: Option<RoleId>,
    pub spam_action_level: ActionLevel,
    pub impersonation_action_level: ActionLevel,
    pub bigotry_action_level: ActionLevel,
    pub ignored_roles: Vec<RoleId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<DbServerConfig> for ServerConfig {
    type Error = anyhow::Error;

    fn try_from(db_server_config: DbServerConfig) -> Result<Self, Self::Error> {
        let DbServerConfig {
            guild_id,
            log_channel_id,
            ping_users,
            ping_role,
            spam_action_level,
            impersonation_action_level,
            bigotry_action_level,
            ignored_roles,
            created_at,
            updated_at,
        } = db_server_config;

        let guild_id = GuildId::from_str(&guild_id)?;
        let log_channel_id = log_channel_id
            .map(|c| ChannelId::from_str(&c))
            .transpose()?;
        let ping_role = ping_role.map(|r| RoleId::from_str(&r)).transpose()?;
        let ignored_roles = ignored_roles
            .into_iter()
            .map(|r| RoleId::from_str(&r).map_err(anyhow::Error::from))
            .collect::<anyhow::Result<Vec<_>>>()?;

        let spam_action_level = ActionLevel::try_from(spam_action_level)?;
        let impersonation_action_level = ActionLevel::try_from(impersonation_action_level)?;
        let bigotry_action_level = ActionLevel::try_from(bigotry_action_level)?;

        Ok(ServerConfig {
            guild_id,
            log_channel_id,
            ping_users,
            ping_role,
            spam_action_level,
            impersonation_action_level,
            bigotry_action_level,
            ignored_roles,
            created_at,
            updated_at,
        })
    }
}

#[derive(Debug)]
pub struct ServerConfigComplete {
    pub guild: PartialGuild,
    pub server_config: ServerConfig,
    pub users: Vec<UserId>,
}

impl ServerConfigComplete {
    pub async fn try_from_server_config(
        server_config: ServerConfig,
        ctx: AppContext<'_>,
    ) -> anyhow::Result<Self> {
        let user_future =
            UserModelController::get_by_guild(&ctx.data().db_pool, server_config.guild_id);

        let partial_future = server_config.guild_id.to_partial_guild(ctx).map_err(|e| {
            anyhow::Error::new(e).context(format!(
                "Failed to upgrade server config for {}",
                server_config.guild_id
            ))
        });

        let (users, guild) = tokio::try_join!(user_future, partial_future)?;

        Ok(Self {
            guild,
            server_config,
            users: users.into_iter().map(|u| u.user_id).collect::<Vec<_>>(),
        })
    }

    pub fn to_embed(&self, interaction_user: &SerenityUser) -> CreateEmbed {
        let server_id = format::inline_code(self.guild.id.to_string());

        let guild_users = self
            .users
            .iter()
            .map(|u| format!("<@{}>", u))
            .collect::<Vec<_>>()
            .join("\n");

        let log_channel = self
            .server_config
            .log_channel_id
            .map(|c| c.mention().to_string())
            .unwrap_or(String::from("Not set."));

        let ping_role = self
            .server_config
            .ping_role
            .map(|r| r.mention().to_string())
            .unwrap_or(String::from("Not set."));

        let spam = self.server_config.spam_action_level.to_string();
        let impersonation = self.server_config.impersonation_action_level.to_string();
        let bigotry = self.server_config.bigotry_action_level.to_string();

        let ignored_roles = if self.server_config.ignored_roles.is_empty() {
            String::from("None set.")
        } else {
            self.server_config
                .ignored_roles
                .iter()
                .map(|r| r.mention().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };

        let created_at = format::display_time(self.server_config.created_at);
        let updated_at = format::display_time(self.server_config.updated_at);

        embeds::CreateJanitorEmbed::new(interaction_user)
            .into_embed()
            .title(format!("Server Config for {}", &self.guild.name))
            .field("Server ID", server_id, false)
            .field("Whitelisted Admins", guild_users, false)
            .field("Log Channel", log_channel, false)
            .field("Ping Role", ping_role, false)
            .field("Spam Action Level", spam, false)
            .field("Impersonation Action Level", impersonation, false)
            .field("Bigotry Action Level", bigotry, false)
            .field("Ignored Roles", ignored_roles, false)
            .field("Created At", created_at, false)
            .field("Updated At", updated_at, false)
    }
}

pub struct UpdateServerConfig {
    pub log_channel_id: Option<ChannelId>,
    pub ping_users: Option<bool>,
    pub ping_role: Option<RoleId>,
    pub spam_action_level: Option<ActionLevel>,
    pub impersonation_action_level: Option<ActionLevel>,
    pub bigotry_action_level: Option<ActionLevel>,
    pub ignored_roles: Option<Vec<RoleId>>,
}

pub struct ServerConfigModelController;

impl ServerConfigModelController {
    pub async fn create_default_if_not_exists(
        pg_pool: &PgPool,
        guild_id: GuildId,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO server_configs (guild_id)
            VALUES ($1)
            ON CONFLICT (guild_id) DO NOTHING;
            "#,
        )
        .bind(guild_id.to_string())
        .execute(pg_pool)
        .await?;

        Ok(())
    }

    /// Gets a guilds's [ServerConfig] by its [GuildId] from the database.
    pub async fn get_by_guild_id(
        pg_pool: &PgPool,
        guild_id: GuildId,
    ) -> anyhow::Result<Option<ServerConfig>> {
        sqlx::query_as::<_, DbServerConfig>("SELECT * FROM server_configs WHERE guild_id = $1;")
            .bind(guild_id.to_string())
            .fetch_optional(pg_pool)
            .await?
            .map(ServerConfig::try_from)
            .transpose()
    }

    /// Gets multiple guild's [ServerConfig]s by their [GuildId]s from the database.
    pub async fn get_multiple_by_guild_id(
        pg_pool: &PgPool,
        guild_ids: &[GuildId],
    ) -> anyhow::Result<Vec<ServerConfig>> {
        let guild_ids: Vec<String> = guild_ids.iter().map(|id| id.to_string()).collect();

        sqlx::query_as::<_, DbServerConfig>(
            "SELECT * FROM server_configs WHERE guild_id = ANY($1::text[]);",
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

    pub async fn update(
        pg_pool: &PgPool,
        guild_id: GuildId,
        update: UpdateServerConfig,
    ) -> anyhow::Result<ServerConfig> {
        let previous = sqlx::query_as::<_, DbServerConfig>(
            "SELECT * FROM server_configs WHERE guild_id = $1;",
        )
        .bind(guild_id.to_string())
        .fetch_optional(pg_pool)
        .await?;

        if previous.is_none() {
            return Err(anyhow::anyhow!("Server config does not exist"));
        }

        let previous = previous.unwrap();

        let log_channel_id_str = update
            .log_channel_id
            .map(|c| Some(c.to_string()))
            .unwrap_or(previous.log_channel_id);

        let ping_users = update.ping_users.unwrap_or(previous.ping_users);

        let ping_role = update
            .ping_role
            .map(|r| Some(r.to_string()))
            .unwrap_or(previous.ping_role);

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

        let ignored_roles = update
            .ignored_roles
            .map(|i| {
                i.iter()
                    .map(|role_id| role_id.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or(previous.ignored_roles);

        let db_config = sqlx::query_as::<_, DbServerConfig>(
            r#"
            UPDATE server_configs
            SET log_channel_id = $2,
                ping_users = $3,
                ping_role = $4,
                spam_action_level = $5,
                impersonation_action_level = $6,
                bigotry_action_level = $7,
                ignored_roles = $8,
                updated_at = now()
            WHERE guild_id = $1
            RETURNING *;
            "#,
        )
        .bind(guild_id.to_string())
        .bind(log_channel_id_str)
        .bind(ping_users)
        .bind(ping_role)
        .bind(spam_action_level)
        .bind(impersonation_action_level)
        .bind(bigotry_action_level)
        .bind(&ignored_roles)
        .fetch_one(pg_pool)
        .await
        .context(format!(
            "Failed to update server config for guild {guild_id}"
        ))?;

        db_config.try_into()
    }

    pub async fn delete(pg_pool: &PgPool, guild_id: GuildId) -> anyhow::Result<ServerConfig> {
        let server_config = sqlx::query_as::<_, DbServerConfig>(
            "DELETE FROM server_configs WHERE guild_id = $1 RETURNING *;",
        )
        .bind(guild_id.to_string())
        .fetch_one(pg_pool)
        .await?;

        ServerConfig::try_from(server_config)
    }

    pub async fn delete_if_needed(pg_pool: &PgPool, guild_id: GuildId) -> anyhow::Result<bool> {
        let sql = r#"
            WITH user_check AS (
                SELECT EXISTS(SELECT 1 FROM users WHERE $1 = ANY(guild_ids)) AS exists
            )
            DELETE FROM server_configs
                WHERE guild_id = $1
                    AND
                (SELECT NOT exists FROM user_check)
            RETURNING TRUE;
        "#;

        let deleted = sqlx::query_scalar::<_, String>(sql)
            .bind(guild_id.to_string())
            .fetch_optional(pg_pool)
            .await
            .context(format!("Failed to check and delete guild {guild_id}"))?
            .is_some();

        tracing::info!("Deleted unused server config for guild {guild_id}");

        Ok(deleted)
    }
}
