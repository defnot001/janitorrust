use serenity::all::{CreateEmbed, CreateEmbedFooter};

use crate::Context;

pub fn create_default_embed(ctx: &Context<'_>) -> CreateEmbed {
    let footer = CreateEmbedFooter::new(format!(
        "Requested by {}",
        ctx.author()
            .to_owned()
            .global_name
            .unwrap_or(ctx.author().to_owned().name)
    ))
    .icon_url(ctx.author().avatar_url().unwrap_or_default());

    CreateEmbed::new()
        .color(3_517_048)
        .footer(footer)
        .timestamp(chrono::Utc::now())
}
