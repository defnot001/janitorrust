// #![allow(dead_code, unused_variables)]
#![warn(clippy::needless_borrow)]

mod broadcast;
mod commands;
mod database;
mod honeypot;
mod util;

use std::sync::Arc;

use commands::{adminconfig, adminlist, badactor, config, scores, user};
use dashmap::DashSet;
use honeypot::channels::HoneypotChannels;
use honeypot::message::{handle_message, Queue};
use poise::serenity_prelude as serenity;
use serenity::InteractionType;
use sqlx::postgres::PgPoolOptions;

use tokio::sync::Mutex;
use util::config::Config;
use util::logger::Logger;
use util::{error, format};

use crate::database::migrate::migrate_db;
use crate::honeypot::channels::populate_honeypot_channels;

#[derive(Debug)]
pub struct Data {
    pub db_pool: sqlx::PgPool,
    pub config: Config,
    pub queue: Queue,
    pub honeypot_channels: HoneypotChannels,
}

pub type AppContext<'a> = poise::Context<'a, Data, anyhow::Error>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing::subscriber::set_global_default(tracing_subscriber::fmt().compact().finish())?;
    tracing::info!("Successfully set up logging!");

    let config = Config::load()?;
    tracing::info!("Successfully loaded config!");

    Logger::set(config.admin_server_error_log_channel);
    tracing::info!("Successfully initialized the logger!");

    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;
    tracing::info!("Successfully connected to the database!");

    migrate_db(&db_pool).await;

    let intents = serenity::GatewayIntents::GUILDS
        | serenity::GatewayIntents::GUILD_MODERATION
        | serenity::GatewayIntents::GUILD_MESSAGES
        | serenity::GatewayIntents::MESSAGE_CONTENT;

    let token = config.bot_token.clone();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                adminconfig::adminconfig(),
                adminlist::adminlist(),
                config::config(),
                scores::scores(),
                user::user(),
                badactor::badactor(),
            ],
            event_handler: |ctx, event, framework, _data| {
                Box::pin(event_handler(ctx, event, framework))
            },
            on_error: |error| {
                Box::pin(async move {
                    error::error_handler(error)
                        .await
                        .expect("Failed to recover from error!");
                })
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                let queue = Arc::new(Mutex::new(Vec::new()));
                let honeypot_channels = Arc::new(DashSet::new());

                Ok(Data {
                    db_pool,
                    config,
                    queue,
                    honeypot_channels,
                })
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client?.start().await?;

    Ok(())
}

async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    framework: poise::FrameworkContext<'_, Data, anyhow::Error>,
) -> Result<(), anyhow::Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            tracing::info!("Logged in as {}!", data_about_bot.user.name);
            ctx.set_activity(Some(serenity::ActivityData {
                name: "bad actors".to_string(),
                kind: serenity::ActivityType::Watching,
                state: None,
                url: None,
            }));

            let db_pool = &framework.user_data.db_pool;
            let honeypot_channels = &framework.user_data.honeypot_channels;

            populate_honeypot_channels(honeypot_channels, db_pool).await;
            tracing::info!("Successfully populated honeypot channels");
        }
        serenity::FullEvent::InteractionCreate { interaction, .. } => {
            if interaction.kind() != InteractionType::Command {
                return Ok(());
            }

            if let Some(command) = interaction.as_command() {
                match command.guild_id {
                    Some(guild_id) => match guild_id.to_partial_guild(ctx).await {
                        Ok(partial_guild) => {
                            let message = format!(
                                "{} used /{} in {}",
                                format::display(&command.user),
                                command.data.name,
                                format::display(&partial_guild)
                            );

                            tracing::info!(message);
                        }
                        Err(e) => {
                            tracing::error!("Error getting partial guild: {e}")
                        }
                    },
                    None => {
                        let message = format!(
                            "{} used /{} outside of a guild.",
                            format::display(&command.user),
                            command.data.name,
                        );

                        tracing::info!(message);
                    }
                };
            }
        }
        serenity::FullEvent::Message { new_message } => {
            handle_message(ctx, framework, new_message).await;
        }
        _ => {}
    }
    Ok(())
}
