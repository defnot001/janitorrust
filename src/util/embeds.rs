use poise::serenity_prelude as serenity;
use serenity::{Colour, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, User};

use crate::AppContext;

use super::format;

#[derive(Default, Copy, Clone, poise::ChoiceParameter)]
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

pub struct CreateJanitorEmbed(CreateEmbed);

impl CreateJanitorEmbed {
    pub fn new(interaction_user: &User) -> Self {
        let name = interaction_user
            .global_name
            .as_deref()
            .unwrap_or(interaction_user.name.as_str());

        let footer = CreateEmbedFooter::new(format!("Requested by {name}")).icon_url(
            interaction_user
                .static_avatar_url()
                .unwrap_or(interaction_user.default_avatar_url()),
        );

        let embed = CreateEmbed::new()
            .color(EmbedColor::Kiwi)
            .footer(footer)
            .timestamp(chrono::Utc::now());

        Self(embed)
    }

    /// Sets the User's avatar or the default avatar as the embeds thumbnail if they don't have one.
    pub fn avatar_thumbnail(self, user: &User) -> Self {
        let thumbnail = user
            .static_avatar_url()
            .unwrap_or(user.default_avatar_url());

        Self(self.0.thumbnail(thumbnail))
    }

    /// Tries to set the bot as an author for the embed and attempts to put its avatar as the author icon.
    /// If something fails, the embed won't have an author.
    pub async fn bot_author(self, ctx: AppContext<'_>) -> Self {
        let Some(bot_user) = ctx.framework().bot_id.to_user(&ctx).await.ok() else {
            return self;
        };

        let icon_url = bot_user
            .static_avatar_url()
            .unwrap_or(bot_user.default_avatar_url());

        let embed_author =
            CreateEmbedAuthor::new(format::display_username(&bot_user)).icon_url(icon_url);

        Self(self.0.author(embed_author))
    }

    pub fn into_embed(self) -> CreateEmbed {
        self.0
    }
}

impl From<CreateJanitorEmbed> for CreateEmbed {
    fn from(janitor_embed: CreateJanitorEmbed) -> Self {
        janitor_embed.0
    }
}
