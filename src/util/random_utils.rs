use std::{num::NonZeroU64, str::FromStr};

use serenity::all::{CacheHttp, CreateEmbed, PartialGuild, User, UserId};

pub fn parse_snowflake(snowflake: impl Into<String>) -> anyhow::Result<std::num::NonZeroU64> {
    NonZeroU64::from_str(snowflake.into().as_str()).map_err(anyhow::Error::new)
}

pub async fn get_users(
    user_ids: Vec<UserId>,
    cache_http: &impl CacheHttp,
) -> anyhow::Result<Vec<User>> {
    let mut users = Vec::new();

    for user_id in user_ids {
        let user = user_id.to_user(cache_http).await?;

        users.push(user);
    }

    Ok(users)
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

pub fn maybe_set_guild_thumbnail(embed: CreateEmbed, guild: &PartialGuild) -> CreateEmbed {
    if let Some(url) = guild.icon_url() {
        embed.thumbnail(url)
    } else {
        embed
    }
}
