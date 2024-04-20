use poise::serenity_prelude as serenity;
use serenity::{User, UserId};

use crate::{
    database::admin_model_controller::AdminModelController,
    util::{error::respond_error, format::fdisplay},
    Context,
};

/// Get the list of admins of this bot.
#[poise::command(slash_command, guild_only = true)]
pub async fn adminlist(ctx: Context<'_>) -> anyhow::Result<()> {
    ctx.defer().await?;

    let admins = match AdminModelController::get_all(&ctx.data().db_pool).await {
        Ok(admins) => admins.into_iter().map(|a| a.id).collect::<Vec<UserId>>(),
        Err(e) => {
            let msg = "Failed get the admins from the from the database";
            return respond_error(msg, e, &ctx).await;
        }
    };

    let mut users = Vec::new();

    for admin in admins {
        users.push(admin.to_user(&ctx).await?)
    }

    let users = users
        .into_iter()
        .map(|u| fdisplay(&u))
        .collect::<Vec<String>>()
        .join("\n");

    ctx.say(users).await?;

    Ok(())
}
