use crate::{database::admin_model_controller::AdminModelController, Context};

pub async fn is_admin(ctx: &Context<'_>) -> bool {
    if let Ok(db_user) = AdminModelController::get(&ctx.data().db_pool, &ctx.author().id).await {
        db_user.is_some()
    } else {
        false
    }
}

pub fn is_in_admin_server(ctx: &Context<'_>) -> bool {
    if let Some(guild_id) = ctx.guild_id() {
        guild_id == ctx.data().config.admins_server_id
    } else {
        false
    }
}
