use std::fmt::Display;
use std::str::FromStr;

use chrono::{DateTime, NaiveDateTime, Utc};
use poise::serenity_prelude as serenity;
use serenity::{
    CacheHttp, CreateAttachment, CreateEmbed, CreateEmbedFooter, GuildId, Mentionable,
    PartialGuild, User as SerenityUser, User, UserId,
};
use sqlx::{FromRow, PgPool};

use crate::util::embeds::EmbedColor;
use crate::util::{format, screenshot};
use crate::Logger;

#[derive(Debug, Copy, Clone)]
pub enum BadActorType {
    Spam,
    Impersonation,
    Bigotry,
    Honeypot,
}

impl Display for BadActorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spam => write!(f, "spam"),
            Self::Impersonation => write!(f, "impersonation"),
            Self::Bigotry => write!(f, "bigotry"),
            Self::Honeypot => write!(f, "honeypot"),
        }
    }
}

impl FromStr for BadActorType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "spam" => Ok(Self::Spam),
            "impersonation" => Ok(Self::Impersonation),
            "bigotry" => Ok(Self::Bigotry),
            "honeypot" => Ok(Self::Honeypot),
            _ => anyhow::bail!("Invalid actor type: {}", s),
        }
    }
}

#[derive(Debug, poise::ChoiceParameter)]
pub enum BadActorTypeChoice {
    Spam,
    Impersonation,
    Bigotry,
}

#[derive(Debug, FromRow)]
struct DbBadActor {
    id: i32,
    user_id: String,
    is_active: bool,
    actor_type: String,
    originally_created_in: String,
    screenshot_proof: Option<String>,
    explanation: Option<String>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    last_changed_by: String,
}

#[derive(Debug)]
pub struct BadActor {
    pub id: i32,
    pub user_id: UserId,
    pub is_active: bool,
    pub actor_type: BadActorType,
    pub origin_guild_id: GuildId,
    pub screenshot_proof: Option<String>,
    pub explanation: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub updated_by_user_id: UserId,
}

#[derive(Debug)]
pub struct BroadcastEmbedOptions<'a> {
    pub origin_guild_id: GuildId,
    pub origin_guild: Option<PartialGuild>,
    pub report_author: &'a User,
    pub bot_id: UserId,
}

impl BadActor {
    pub async fn user(&self, cache_http: impl CacheHttp) -> Option<SerenityUser> {
        self.user_id.to_user(cache_http).await.ok()
    }

    /// Infailliable method to get a broadcast embed from a bad actor.
    pub async fn to_broadcast_embed<'a>(
        &self,
        cache_http: impl CacheHttp,
        options: BroadcastEmbedOptions<'a>,
        colour: EmbedColor,
    ) -> (CreateEmbed, Option<CreateAttachment>) {
        let BroadcastEmbedOptions {
            origin_guild_id,
            origin_guild,
            report_author,
            bot_id,
        } = options;

        let explanation = self
            .explanation
            .clone()
            .unwrap_or("No explanation provided.".to_string());

        let target_user = self.user(&cache_http).await;

        let title = target_user
            .clone()
            .map(|u| {
                format!(
                    "{} (`{}`)",
                    u.global_name.clone().unwrap_or(u.name.clone()),
                    self.user_id
                )
            })
            .unwrap_or(format!("Unknown User (`{}`)", self.user_id));

        let author = format!("{} (`{}`)", report_author.mention(), report_author.id);

        let display_guild = origin_guild
            .as_ref()
            .map(format::fdisplay)
            .unwrap_or(origin_guild_id.to_string());

        let embed = CreateEmbed::default()
            .title(title)
            .color(colour)
            .timestamp(Utc::now())
            .field("Report ID", self.id.to_string(), true)
            .field("Active", format::display_bool(self.is_active), true)
            .field("Type", self.actor_type.to_string(), true)
            .field("Explanation", explanation, false)
            .field("Server of Origin", display_guild, false)
            .field("Last Updated By", author, false);

        // add thumbnail
        let embed = match target_user {
            None => embed,
            Some(u) => embed.thumbnail(u.static_avatar_url().unwrap_or(u.default_avatar_url())),
        };

        // add footer
        let embed = match bot_id.to_user(&cache_http).await {
            Ok(bot_user) => embed.footer(
                CreateEmbedFooter::new(
                    bot_user
                        .global_name
                        .clone()
                        .unwrap_or(bot_user.name.clone()),
                )
                .icon_url(
                    bot_user
                        .static_avatar_url()
                        .unwrap_or(bot_user.default_avatar_url()),
                ),
            ),
            Err(e) => {
                let log_msg = "Failed to get bot user";
                Logger::get().error(&cache_http, e, log_msg).await;
                embed
            }
        };

        let attachment = match self.screenshot_proof.clone() {
            Some(path) => screenshot::FileManager::get(&path).await.ok(),
            None => None,
        };

        match attachment {
            Some(attachment) => {
                let embed = embed.image(format!("attachment://{}", attachment.filename));

                (embed, Some(attachment))
            }
            None => (embed, None),
        }
    }

    pub fn ban_reason(&self, custom_reason: Option<String>) -> String {
        if let Some(reason) = custom_reason {
            reason
                .replace("{type}", self.actor_type.to_string().as_str())
                .replace("{id}", self.id.to_string().as_str())
        } else {
            format!("Bad Actor {} ({})", self.actor_type, self.id)
        }
    }
}

impl TryFrom<DbBadActor> for BadActor {
    type Error = anyhow::Error;

    fn try_from(db_bad_actor: DbBadActor) -> Result<Self, Self::Error> {
        let DbBadActor {
            id,
            is_active,
            screenshot_proof,
            explanation,
            created_at,
            updated_at,
            ..
        } = db_bad_actor;

        let actor_type = BadActorType::from_str(&db_bad_actor.actor_type)?;
        let user_id = UserId::from_str(&db_bad_actor.user_id)?;
        let origin_guild_id = GuildId::from_str(&db_bad_actor.originally_created_in)?;
        let updated_by_user_id = UserId::from_str(&db_bad_actor.last_changed_by)?;

        let created_at = created_at.and_utc();
        let updated_at = updated_at.and_utc();

        let bad_actor = BadActor {
            id,
            user_id,
            is_active,
            actor_type,
            screenshot_proof,
            explanation,
            created_at,
            updated_at,
            origin_guild_id,
            updated_by_user_id,
        };

        Ok(bad_actor)
    }
}

pub struct CreateBadActorOptions {
    pub user_id: UserId,
    pub actor_type: BadActorType,
    pub screenshot_proof: Option<String>,
    pub explanation: Option<String>,
    pub origin_guild_id: GuildId,
    pub updated_by_user_id: UserId,
}

#[derive(Debug, poise::ChoiceParameter)]
pub enum BadActorQueryType {
    All,
    Active,
    Inactive,
}

pub struct BadActorModelController;

impl BadActorModelController {
    /// Create a new bad actor entry in the database. Returns the newly created bad actor.
    pub async fn create(
        db_pool: &PgPool,
        options: CreateBadActorOptions,
    ) -> anyhow::Result<BadActor> {
        let CreateBadActorOptions {
            user_id,
            actor_type,
            screenshot_proof,
            explanation,
            origin_guild_id,
            updated_by_user_id,
        } = options;

        sqlx::query_as::<_, DbBadActor>(
            r#"
            INSERT INTO bad_actors (user_id, actor_type, originally_created_in, screenshot_proof, explanation, last_changed_by)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *;
            "#,
        )
        .bind(user_id.to_string())
        .bind(actor_type.to_string())
        .bind(origin_guild_id.to_string())
        .bind(screenshot_proof)
        .bind(explanation)
        .bind(updated_by_user_id.to_string())
        .fetch_one(db_pool)
        .await?
        .try_into()
    }

    /// Returns if the given user ID currently has an active case.
    pub async fn has_active_case(db_pool: &PgPool, user_id: UserId) -> bool {
        sqlx::query_as::<_, DbBadActor>(
            "SELECT * FROM bad_actors WHERE user_id = $1 AND is_active = true;",
        )
        .bind(user_id.to_string())
        .fetch_optional(db_pool)
        .await
        .map(|db_bad_actor| db_bad_actor.is_some())
        .unwrap_or(false)
    }

    /// Get all entries for a given discord user ID.
    pub async fn get_by_user_id(
        db_pool: &PgPool,
        user_id: UserId,
    ) -> anyhow::Result<Vec<BadActor>> {
        let db_bad_actors =
            sqlx::query_as::<_, DbBadActor>("SELECT * FROM bad_actors WHERE user_id = $1;")
                .bind(user_id.to_string())
                .fetch_all(db_pool)
                .await?;

        db_bad_actors
            .into_iter()
            .map(BadActor::try_from)
            .collect::<Result<Vec<BadActor>, _>>()
    }

    /// Get a specific bad actor entry by its unique ID.
    pub async fn get_by_id(db_pool: &PgPool, id: i32) -> anyhow::Result<Option<BadActor>> {
        let db_bad_actor =
            sqlx::query_as::<_, DbBadActor>("SELECT * FROM bad_actors WHERE id = $1;")
                .bind(id)
                .fetch_optional(db_pool)
                .await?;

        db_bad_actor.map(BadActor::try_from).transpose()
    }

    /// Deactivate a bad actor entry by its unique ID with the given explanation.
    /// This also updates the `updated_by_user_id` field to the user ID of the user who deactivated the entry.
    pub async fn deavtivate(
        db_pool: &PgPool,
        id: i32,
        explanation: impl Into<String>,
        updated_by_user_id: UserId,
    ) -> anyhow::Result<BadActor> {
        let updated_db_bad_actor = sqlx::query_as::<_, DbBadActor>(
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
        .bind(id)
        .bind(explanation.into())
        .bind(updated_by_user_id.to_string())
        .fetch_one(db_pool)
        .await?;

        updated_db_bad_actor.try_into()
    }

    /// Get the most recent bad actor entries with the given limit and query type. Defaults to `BadActorQueryType::All`.
    pub async fn get_by_type(
        db_pool: &PgPool,
        limit: i64,
        query_type: Option<BadActorQueryType>,
    ) -> anyhow::Result<Vec<BadActor>> {
        let query_str = match query_type.unwrap_or(BadActorQueryType::All) {
            BadActorQueryType::All => "SELECT * FROM bad_actors ORDER BY created_at DESC LIMIT $1;",
            BadActorQueryType::Active => {
                "SELECT * FROM bad_actors WHERE is_active = true ORDER BY created_at DESC LIMIT $1"
            }
            BadActorQueryType::Inactive => {
                "SELECT * FROM bad_actors WHERE is_active = false ORDER BY created_at DESC LIMIT $1"
            }
        };

        let db_bad_actors = sqlx::query_as::<_, DbBadActor>(query_str)
            .bind(limit)
            .fetch_all(db_pool)
            .await?;

        db_bad_actors
            .into_iter()
            .map(BadActor::try_from)
            .collect::<Result<Vec<BadActor>, _>>()
    }

    pub async fn delete(pg_pool: &PgPool, id: i32) -> anyhow::Result<BadActor> {
        let deleted_db_bad_actor =
            sqlx::query_as::<_, DbBadActor>("DELETE FROM bad_actors WHERE id = $1 RETURNING *;")
                .bind(id)
                .fetch_one(pg_pool)
                .await?;

        tracing::info!("Deleted bad actor entry with ID {id} from the database.");

        deleted_db_bad_actor.try_into()
    }

    /// Update the screenshot proof of a bad actor entry by its unique ID.
    /// This also updates the `last_changed_by` field to the user ID of the user who updated the entry.
    pub async fn update_screenshot(
        pg_pool: &PgPool,
        id: i32,
        updated_by_user_id: UserId,
        screenshot_path: impl Into<String>,
    ) -> anyhow::Result<BadActor> {
        let updated_db_bad_actor = sqlx::query_as::<_, DbBadActor>(
            r#"
            UPDATE bad_actors
            SET
                screenshot_proof = $2,
                last_updated_by = $3,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING *;
            "#,
        )
        .bind(id)
        .bind(screenshot_path.into())
        .bind(updated_by_user_id.to_string())
        .fetch_one(pg_pool)
        .await?;

        updated_db_bad_actor.try_into()
    }

    pub async fn update_explanation(
        pg_pool: &PgPool,
        id: i32,
        updated_by_user_id: UserId,
        explanation: impl Into<String>,
    ) -> anyhow::Result<BadActor> {
        let updated_db_bad_actor = sqlx::query_as::<_, DbBadActor>(
            r#"
            UPDATE bad_actors
            SET
                explanation = $2,
                last_updated_by = $3,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING *;
            "#,
        )
        .bind(id)
        .bind(explanation.into())
        .bind(updated_by_user_id.to_string())
        .fetch_one(pg_pool)
        .await?;

        updated_db_bad_actor.try_into()
    }
}
