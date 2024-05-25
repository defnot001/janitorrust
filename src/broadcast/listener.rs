use futures::stream::FuturesUnordered;
use futures::StreamExt;
use poise::serenity_prelude as serenity;
use serenity::{GuildChannel, GuildId};

use crate::database::controllers::serverconfig_model_controller::{
    ServerConfig, ServerConfigComplete, ServerConfigModelController,
};
use crate::util::logger::Logger;
use crate::Context as AppContext;

#[derive(Debug)]
pub struct BroadcastListener {
    pub config: ServerConfigComplete,
    pub log_channel: GuildChannel,
}

pub async fn get_valid_listeners(ctx: AppContext<'_>) -> anyhow::Result<Vec<BroadcastListener>> {
    let mut config_futures = ServerConfigModelController::get_all(&ctx.data().db_pool)
        .await?
        .into_iter()
        .map(|server_config| async { get_valid_logchannel(ctx, server_config).await })
        .collect::<FuturesUnordered<_>>();

    let mut valid_configs = Vec::new();

    while let Some((guild_id, config_result, log_channel)) = config_futures.next().await {
        match config_result {
            Ok(config) => {
                if let Some(c) = log_channel {
                    valid_configs.push(BroadcastListener {
                        config,
                        log_channel: c,
                    });
                }
            }
            Err(e) => {
                let log_future = async {
                    let log_msg = format!("Failed to upgrade config for {}. Skipping their server for broadcasting: {e}", guild_id);
                    Logger::get().warn(ctx, log_msg).await;
                };
                log_future.await;
            }
        }
    }

    Ok(valid_configs)
}

async fn get_valid_logchannel(
    ctx: AppContext<'_>,
    server_config: ServerConfig,
) -> (
    GuildId,
    anyhow::Result<ServerConfigComplete>,
    Option<GuildChannel>,
) {
    let server_id = server_config.guild_id;

    let Some(log_channel) = server_config.log_channel_id else {
        let err = Err(anyhow::anyhow!(
            "There is no log channel defined for {}",
            server_config.guild_id
        ));

        return (server_id, err, None);
    };

    let Ok(log_channel) = log_channel.to_channel(ctx).await else {
        let err = Err(anyhow::anyhow!(
            "Cannot get log channel for {}",
            server_config.guild_id
        ));

        return (server_id, err, None);
    };

    let Some(log_channel) = log_channel.guild() else {
        let err = Err(anyhow::anyhow!(
            "Log channel for {} is not a guild channel",
            server_config.guild_id
        ));

        return (server_id, err, None);
    };

    if !log_channel.is_text_based() {
        let err = Err(anyhow::anyhow!(
            "Log channel for {} is not a text channel",
            server_config.guild_id
        ));

        return (server_id, err, None);
    }

    let complete = ServerConfigComplete::try_from_server_config(server_config, ctx).await;

    (server_id, complete, Some(log_channel))
}
