use crate::{
    database::admin_model_controller::AdminModelController, util::permissions::is_admin, Context,
};

#[macro_export]
macro_rules! assert_admin {
    ($ctx:expr) => {
        if !$crate::util::permissions::is_admin($ctx).await {
            $ctx.say("This command can only be used by an admin.")
                .await?;
            return Ok(());
        }
    };
}

#[macro_export]
macro_rules! assert_admin_server {
    ($ctx:expr) => {
        if !$crate::util::permissions::is_in_admin_server($ctx) {
            $ctx.say("This command can only be used in the admin server.")
                .await?;
            return Ok(());
        }
    };
}
