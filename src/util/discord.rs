use std::num::NonZeroU64;

use futures::future;
use poise::serenity_prelude::{CacheHttp, GuildId, PartialGuild, User, UserId};

#[async_trait::async_trait]
pub trait ToEntity {
    type Entity;

    async fn to_entity(&self, cache_http: impl CacheHttp) -> serenity::Result<Self::Entity>;
}

#[async_trait::async_trait]
impl ToEntity for UserId {
    type Entity = User;

    async fn to_entity(&self, cache_http: impl CacheHttp) -> serenity::Result<User> {
        self.to_user(&cache_http).await
    }
}

#[async_trait::async_trait]
impl ToEntity for GuildId {
    type Entity = PartialGuild;

    async fn to_entity(&self, cache_http: impl CacheHttp) -> serenity::Result<PartialGuild> {
        self.to_partial_guild(&cache_http).await
    }
}

pub async fn get_entities<T: ToEntity + Sync>(
    cache_http: impl CacheHttp,
    ids: &[T],
) -> serenity::Result<Vec<T::Entity>> {
    let async_iter = ids.iter().map(|id| id.to_entity(&cache_http));
    future::try_join_all(async_iter).await
}

pub fn parse_snowflake(str: &str) -> anyhow::Result<NonZeroU64> {
    NonZeroU64::new(str.parse::<u64>()?).ok_or(anyhow::anyhow!(
        "parsing error: snowflake cannot be zero but got string: {str}"
    ))
}
