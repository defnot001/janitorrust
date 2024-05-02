use poise::serenity_prelude as serenity;
use serenity::UserId;

use crate::database::admin_model_controller::AdminModelController;
use crate::util::format;
use crate::{assert_user, oops};
use crate::{Context as AppContext, Logger};

/// Get the list of admins of this bot.
#[poise::command(slash_command, guild_only = true)]
pub async fn adminlist(ctx: AppContext<'_>) -> anyhow::Result<()> {
    assert_user!(ctx);

    ctx.defer().await?;

    let admins = match AdminModelController::get_all(&ctx.data().db_pool).await {
        Ok(admins) => admins.into_iter().map(|a| a.id).collect::<Vec<UserId>>(),
        Err(e) => {
            let msg = "Failed get the admins from the from the database";
            Logger::get().error(ctx, e, msg).await;
            oops!(ctx, msg);
        }
    };

    let mut users = Vec::new();

    for admin in admins {
        users.push(admin.to_user(&ctx).await?)
    }

    let users = users
        .into_iter()
        .map(|u| format::fdisplay(&u))
        .collect::<Vec<String>>()
        .join("\n");

    ctx.say(users).await?;

    Ok(())
}
