use std::{num::NonZeroU64, str::FromStr};

use serenity::all::{CacheHttp, CreateEmbed, GuildId, PartialGuild, RoleId, User, UserId};

pub fn parse_snowflake(snowflake: impl Into<String>) -> anyhow::Result<std::num::NonZeroU64> {
    NonZeroU64::from_str(snowflake.into().as_str()).map_err(anyhow::Error::new)
}

pub async fn get_users(
    user_ids: Vec<UserId>,
    cache_http: &impl CacheHttp,
) -> anyhow::Result<Vec<User>> {
    let mut users = Vec::new();

    for user_id in user_ids {
        tracing::debug!("Fetching user {user_id}");
        let user = user_id.to_user(cache_http).await?;
        users.push(user);
    }

    Ok(users)
}

pub async fn get_guilds(
    guild_ids: &[GuildId],
    cache_http: &impl CacheHttp,
) -> anyhow::Result<Vec<PartialGuild>> {
    let mut guilds = Vec::new();

    for guild_id in guild_ids {
        tracing::debug!("Fetching guild {guild_id}");
        let guild = guild_id.to_partial_guild(cache_http).await?;
        guilds.push(guild);
    }

    Ok(guilds)
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
