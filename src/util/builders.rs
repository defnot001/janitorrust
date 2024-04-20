use serenity::all::{CreateEmbed, CreateEmbedFooter, User};

pub fn create_default_embed(interaction_user: &User) -> CreateEmbed {
    let name = if let Some(global) = interaction_user.global_name.clone() {
        global
    } else {
        interaction_user.name.clone()
    };

    let footer = CreateEmbedFooter::new(format!("Requested by {}", name)).icon_url(
        interaction_user
            .static_avatar_url()
            .unwrap_or(interaction_user.default_avatar_url()),
    );

    CreateEmbed::new()
        .color(3_517_048)
        .footer(footer)
        .timestamp(chrono::Utc::now())
}
