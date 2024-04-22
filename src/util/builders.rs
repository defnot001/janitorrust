use serenity::all::{Colour, CreateEmbed, CreateEmbedFooter, User};

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

#[derive(Default, poise::ChoiceParameter)]
pub enum EmbedColor {
    #[default]
    Kiwi = 0x35AA78,
    Black = 0x000000,
    Gray = 0xBEBEBE,
    White = 0xFFFFFF,
    Blue = 0x0000FF,
    Cyan = 0x00FFFF,
    Green = 0x00FF00,
    Orange = 0xFFA500,
    Coral = 0xFF7F50,
    Red = 0xFF0000,
    DeepPink = 0xFF1493,
    Purple = 0xA020F0,
    Magenta = 0xFF00FF,
    Yellow = 0xFFFF00,
    Gold = 0xFFD700,
    None = 0x2F3136,
}

impl From<EmbedColor> for Colour {
    fn from(colour: EmbedColor) -> Self {
        Colour(colour as u32)
    }
}
