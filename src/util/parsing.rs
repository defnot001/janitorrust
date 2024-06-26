use std::num::NonZeroU64;

use poise::serenity_prelude as serenity;
use serenity::{GuildId, RoleId};

fn parse_ids(str: &str) -> anyhow::Result<Vec<NonZeroU64>> {
    str.split(',')
        .map(|id| match id.trim().parse::<u64>() {
            Ok(parsed) => {
                NonZeroU64::new(parsed).ok_or(anyhow::Error::msg("Snowflake cannot be zero"))
            }
            Err(e) => anyhow::bail!(e),
        })
        .collect::<anyhow::Result<Vec<_>>>()
}

pub fn parse_guild_ids(str: &str) -> anyhow::Result<Vec<GuildId>> {
    let ids = parse_ids(str)?
        .iter()
        .map(|&id| GuildId::from(id))
        .collect::<Vec<_>>();

    Ok(ids)
}

pub fn parse_role_ids(str: &str) -> anyhow::Result<Vec<RoleId>> {
    let ids = parse_ids(str)?
        .iter()
        .map(|&id| RoleId::from(id))
        .collect::<Vec<_>>();

    Ok(ids)
}
