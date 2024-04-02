#![allow(dead_code, unused)]

mod commands;
mod config;
mod database;
mod format;
mod util;

use ::serenity::all::InteractionType;
use commands::adminconfig;
use poise::serenity_prelude as serenity;
use sqlx::postgres::PgPoolOptions;

pub struct Data {
    pub db_pool: sqlx::PgPool,
}
pub type Context<'a> = poise::Context<'a, Data, anyhow::Error>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing::subscriber::set_global_default(tracing_subscriber::fmt().compact().finish())?;
    tracing::info!("Successfully set up logging!");

    let config = config::Config::load()?;
    tracing::info!("Successfully loaded config!");

    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;
    tracing::info!("Successfully connected to the database!");

    let intents = serenity::GatewayIntents::GUILDS | serenity::GatewayIntents::GUILD_MODERATION;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![adminconfig::adminconfig()],
            event_handler: |ctx, event, framework, _data| {
                Box::pin(event_handler(ctx, event, framework))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data { db_pool })
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(config.bot_token, intents)
        .framework(framework)
        .await;

    client?.start().await?;

    Ok(())
}

async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, anyhow::Error>,
) -> Result<(), anyhow::Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            tracing::info!("Logged in as {}!", data_about_bot.user.name);
            ctx.set_activity(Some(serenity::ActivityData {
                name: "bad actors".to_string(),
                kind: serenity::ActivityType::Watching,
                state: None,
                url: None,
            }))
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
        _ => {}
    }
    Ok(())
}
