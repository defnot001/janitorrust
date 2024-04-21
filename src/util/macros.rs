#[macro_export]
macro_rules! assert_admin {
    ($ctx:ident) => {
        let Ok(Some(_)) = $crate::database::admin_model_controller::AdminModelController::get(
            &$ctx.data().db_pool,
            &$ctx.author().id,
        )
        .await
        else {
            $ctx.say("This command can only be used by an admin.")
                .await?;
            return Ok(());
        };
    };
}

#[macro_export]
macro_rules! assert_admin_server {
    ($ctx:ident) => {
        let Some(guild_id) = $ctx.guild_id() else {
            $ctx.say("This command can only be used in a server.")
                .await?;

            return Ok(());
        };

        if guild_id != $ctx.data().config.admins_server_id {
            $ctx.say("This command can only be used in the admin server.")
                .await?;
            return Ok(());
        }
    };
}

#[macro_export]
macro_rules! assert_user {
    ($ctx:ident) => {
        let Some(guild_id) = $ctx.guild_id() else {
            $ctx.say("This command can only be used in a server.")
                .await?;
            return Ok(());
        };

        let Ok(Some(user)) = $crate::database::user_model_controller::UserModelController::get(
            &$ctx.data().db_pool,
            $ctx.author().id,
        )
        .await
        else {
            $ctx.say("You are not allowed to use this command.").await?;
            return Ok(());
        };
    };
}

#[macro_export]
macro_rules! assert_user_server {
    ($ctx:ident) => {
        let Some(guild_id) = $ctx.guild_id() else {
            $ctx.say("This command can only be used in a server.")
                .await?;
            return Ok(());
        };

        let Ok(Some(user)) = $crate::database::user_model_controller::UserModelController::get(
            &$ctx.data().db_pool,
            $ctx.author().id,
        )
        .await
        else {
            $ctx.say("You are not allowed to use this command.").await?;
            return Ok(());
        };

        if !user.servers.contains(&guild_id) {
            $ctx.say("You are not allowed to use this command here.")
                .await?;
            return Ok(());
        }
    };
}
