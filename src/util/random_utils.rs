use std::{num::NonZeroU64, str::FromStr};

use anyhow::Context;
use futures::future;
use poise::serenity_prelude as serenity;
use serenity::{CacheHttp, GuildId, PartialGuild, RoleId, User, UserId};

use crate::Context as AppContext;

pub fn parse_snowflake(snowflake: impl AsRef<str>) -> anyhow::Result<std::num::NonZeroU64> {
    let context = format!("Failed to parse snowflake `{}`", snowflake.as_ref());
    NonZeroU64::from_str(snowflake.as_ref()).map_err(|e| anyhow::Error::from(e).context(context))
}

pub async fn get_bot_user(ctx: AppContext<'_>) -> Option<User> {
    ctx.framework()
        .bot_id
        .to_user(ctx)
        .await
        .context("Failed to get bot user")
        .ok()
}

/// Tries to display the User's global name and gets the username if they don't have one.
pub fn username(user: &User) -> &str {
    user.global_name.as_ref().unwrap_or(&user.name)
}

pub async fn get_users(
    user_ids: Vec<UserId>,
    cache_http: &impl CacheHttp,
) -> anyhow::Result<Vec<User>> {
    future::try_join_all(user_ids.iter().map(|u| u.to_user(cache_http)))
        .await
        .context("Failed to fetch one or more users from the API")
}

pub async fn get_guilds(
    guild_ids: &[GuildId],
    cache_http: &impl CacheHttp,
) -> anyhow::Result<Vec<PartialGuild>> {
    future::try_join_all(guild_ids.iter().map(|g| {
        tracing::debug!("Fetching guild {g}");
        g.to_partial_guild(cache_http)
    }))
    .await
    .context("Failed to fetch one or more guilds from the API")
}

pub fn parse_guild_ids(str: &str) -> anyhow::Result<Vec<GuildId>> {
    str.split(',')
        .map(|id| match id.parse::<u64>() {
            Ok(id) => {
                if let Some(non_zero) = NonZeroU64::new(id) {
                    Ok(GuildId::from(non_zero))
                } else {
                    anyhow::bail!("0 is not a valid guild id")
                }
            }
            Err(e) => anyhow::bail!(e),
        })
        .collect()
}

pub fn parse_role_ids(str: &str) -> anyhow::Result<Vec<RoleId>> {
    str.split(',')
        .map(|id| match id.parse::<u64>() {
            Ok(id) => {
                if let Some(non_zero) = NonZeroU64::new(id) {
                    Ok(RoleId::from(non_zero))
                } else {
                    anyhow::bail!("0 is not a valid role id")
                }
            }
            Err(e) => anyhow::bail!(e),
        })
        .collect()
}

pub fn display_bool(boolean: bool) -> &'static str {
    if boolean {
        "Yes"
    } else {
        "No"
    }
}

#[cfg(test)]
mod tests {
    use super::parse_snowflake;

    #[test]
    fn parse_snowflake_works() {
        let parsed = parse_snowflake("471026181211422721");

        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().get(), 471026181211422721);
    }

    #[test]
    fn parse_snowflake_fails() {
        let parsed = parse_snowflake("0");

        assert!(parsed.is_err());
    }
}
