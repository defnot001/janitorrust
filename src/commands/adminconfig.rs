use serenity::all::GuildId;

use crate::{database::serverconfig_model_controller::ServerConfigComplete, Context};

/// Subcommands for admins to inspect the bot's server configs.
#[poise::command(
    slash_command,
    guild_only = true,
    subcommands("display_configs", "delete_bad_actor"),
    subcommand_required
)]
pub async fn adminconfig(_: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

/// Delete a bad actor from the database
#[poise::command(slash_command)]
async fn display_configs(
    ctx: Context<'_>,
    #[description = "The ID(s) of the server(s) to display the config for. Separate multiple IDs with a comma (,). Max 5."]
    guild_id: String,
) -> anyhow::Result<()> {
    ctx.defer().await?;

    let guild_ids = guild_id
        .split(',')
        .map(|id| id.trim().parse::<u64>())
        .collect::<Result<Vec<u64>, _>>();

    if guild_ids.is_err() {
        ctx.say("Invalid guild ID(s) provided!").await?;
        return Ok(());
    }

    let guild_ids = guild_ids.unwrap();

    if guild_ids.len() > 5 {
        ctx.say("You can only display the  config for up to 5 servers at a time!")
            .await?;
        return Ok(());
    }

    let mut guilds = Vec::new();

    for guild_id in guild_ids {
        if guild_id == 0 {
            ctx.say("Invalid guild ID(s) provided!").await?;
            return Ok(());
        }

        let partial_guild = GuildId::new(guild_id).to_partial_guild(&ctx).await?;

        guilds.push(partial_guild);
    }

    let mut response = String::new();

    for guild in guilds {
        response.push_str(&format!("Server: {}\nConfig: {}\n", guild.name, "config"));
    }

    ctx.say(response).await?;
    Ok(())
}

/// Another subcommand of `parent`
#[poise::command(slash_command)]
async fn delete_bad_actor(ctx: Context<'_>) -> anyhow::Result<()> {
    ctx.defer().await?;
    ctx.say("You invoked the second child command!").await?;
    Ok(())
}

async fn build_server_config_embed(
    config: ServerConfigComplete,
    ctx: &Context<'_>,
) -> anyhow::Result<CreateEmbed> {
    let Ok(guild) = config.server_config.server_id.to_partial_guild(&ctx).await else {
        return Err(anyhow::anyhow!(
            "Failed to retrieve guild with id {} from the API!",
            config.server_config.server_id,
        ));
    };

    let Ok(config_users) = UserModelController::get_by_guild(&ctx.data().db_pool, &guild.id).await
    else {
        return Err(anyhow::anyhow!(
            "Failed to retrieve users for {} from the database!",
            display(&guild),
        ));
    };

    let user_ids = config_users.iter().map(|user| user.id).collect::<Vec<_>>();

    let Ok(discord_users) = get_users(user_ids, &ctx).await else {
        return Err(anyhow::anyhow!(
            "Failed to retrieve users for {} from the API!",
            display(&guild),
        ));
    };

    let log_channel = if let Some(channel_id) = config.server_config.log_channel {
        let Ok(channel) = channel_id.to_channel(&ctx).await else {
            return Err(anyhow::anyhow!(
                "Failed to retrieve log channel with id {} from the API!",
                channel_id,
            ));
        };

        Some(channel)
    } else {
        None
    };

    let mut embed = create_default_embed(ctx)
        .title(format!("Server Config for {}", &guild.name))
        .field("Server ID", inline_code(&guild.id.to_string()), false)
        .field(
            "Whitelisted Admins",
            discord_users
                .into_iter()
                .map(|user| fdisplay(&user))
                .collect::<Vec<_>>()
                .join("\n"),
            false,
        )
        .field(
            "Log Channel",
            log_channel
                .map(|channel| fdisplay(&channel))
                .unwrap_or_else(|| "Not set".to_string()),
            false,
        );

    Ok(embed)
}
