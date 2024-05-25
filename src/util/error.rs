use poise::FrameworkError;

use crate::{util::logger::Logger, Data};

#[allow(clippy::needless_lifetimes)]
pub async fn error_handler<'a>(
    error: FrameworkError<'a, Data, anyhow::Error>,
) -> anyhow::Result<()> {
    let logger = Logger::get();

    match error {
        FrameworkError::Command { error, ctx, .. } => {
            let error_msg = format!("Command error /{}", ctx.command().name);
            logger.error(ctx, error, error_msg).await;

            if let Err(e) = ctx
                .say("There was an error trying to execute that command.")
                .await
            {
                logger.error(ctx, e, "Failed to send error message").await;
            }

            Ok(())
        }
        FrameworkError::CommandPanic { payload, ctx, .. } => {
            let error_msg = format!("Command panic /{}: ", ctx.command().name);

            if let Some(payload) = payload {
                let error = anyhow::Error::msg(error_msg).context(payload);
                logger.error(ctx, error, "Panic").await;
            } else {
                let error = anyhow::Error::msg(error_msg);
                logger.error(ctx, error, "Panic").await;
            }

            if let Err(e) = ctx
                .say("There was an error trying to execute that command.")
                .await
            {
                logger.error(ctx, e, "Failed to send error message").await;
            }

            Ok(())
        }
        FrameworkError::GuildOnly { ctx, .. } => {
            tracing::error!(
                "Guild-only command {} was used outside of a guild.",
                ctx.command().name.clone()
            );

            match ctx
                .reply("This command can only be used in a server.")
                .await
            {
                Ok(_) => Ok(()),
                Err(e) => {
                    tracing::error!("Failed to send error message: {:?}", e);
                    Ok(())
                }
            }
        }
        FrameworkError::SubcommandRequired { ctx } => {
            tracing::error!(
                "Command {} requires a subcommand but none was provided.",
                ctx.command().name.clone()
            );

            match ctx.reply("This command requires a subcommand.").await {
                Ok(_) => Ok(()),
                Err(e) => {
                    tracing::error!("Failed to send error message: {:?}", e);
                    Ok(())
                }
            }
        }
        FrameworkError::EventHandler { error, event, .. } => {
            tracing::error!(
                "Event handler error for {}: {:#?}",
                event.snake_case_name(),
                error
            );

            Ok(())
        }
        FrameworkError::Setup {
            error,
            data_about_bot,
            ..
        } => {
            let username = data_about_bot.user.name.clone();
            tracing::error!("Failed to setup framework for {username}: {:#?}", error);

            Ok(())
        }
        other => {
            tracing::error!("Unhandled framework error: {:?}", other.to_string());

            Ok(())
        }
    }
}
