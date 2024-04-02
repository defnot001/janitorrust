use serenity::all::GuildId;

use crate::Context;

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
