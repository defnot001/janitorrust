use std::borrow::Cow;

use poise::serenity_prelude as serenity;
use serenity::{CacheHttp, GuildId, PartialGuild, User};

use super::discord::get_entities;

#[allow(dead_code)]
pub enum TimestampStyle {
    ShortTime,
    LongTime,
    ShortDate,
    LongDate,
    ShortDateTime,
    LongDateTime,
    Relative,
}

pub trait HasNameAndID {
    fn name(&self) -> &str;
    fn id(&self) -> Cow<str>;
}

impl HasNameAndID for User {
    fn id(&self) -> Cow<str> {
        self.id.to_string().into()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl HasNameAndID for PartialGuild {
    fn id(&self) -> Cow<str> {
        self.id.to_string().into()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub fn display(input: &impl HasNameAndID) -> String {
    format!("{} ({})", input.name(), input.id())
}

pub fn fdisplay(input: &impl HasNameAndID) -> String {
    format!(
        "{} ({})",
        escape_markdown(input.name()),
        inline_code(input.id())
    )
}

pub fn inline_code(input: impl Into<String>) -> String {
    format!("`{}`", input.into())
}

pub fn escape_markdown(input: impl Into<String>) -> String {
    let input = input.into();
    let mut output = String::with_capacity(input.len());

    for c in input.chars() {
        if c.is_ascii_alphanumeric() || c.is_ascii_whitespace() {
            output.push(c)
        } else {
            output.extend(['\\', c])
        }
    }

    output
}

pub fn time(date_time: chrono::DateTime<chrono::Utc>, style: TimestampStyle) -> String {
    let timestamp = date_time.timestamp();

    match style {
        TimestampStyle::ShortTime => format!("<t:{timestamp}:t>"),
        TimestampStyle::LongTime => format!("<t:{timestamp}:T>"),
        TimestampStyle::ShortDate => format!("<t:{timestamp}:d>"),
        TimestampStyle::LongDate => format!("<t:{timestamp}:D>"),
        TimestampStyle::ShortDateTime => format!("<t:{timestamp}:f>"),
        TimestampStyle::LongDateTime => format!("<t:{timestamp}:F>"),
        TimestampStyle::Relative => format!("<t:{timestamp}:R>"),
    }
}

pub fn display_time(date_time: chrono::DateTime<chrono::Utc>) -> String {
    format!(
        "{}\n{}",
        time(date_time, TimestampStyle::LongDate),
        time(date_time, TimestampStyle::Relative)
    )
}

pub fn display_bool(boolean: bool) -> &'static str {
    if boolean {
        "Yes"
    } else {
        "No"
    }
}

/// Tries to display the User's global name and gets the username if they don't have one.
pub fn display_username(user: &User) -> &str {
    user.global_name.as_ref().unwrap_or(&user.name)
}

// pub async fn display_guilds(partial_guilds: &[PartialGuild], use_markdown: bool) -> String {
//     partial_guilds
//         .into_iter()
//         .map(|g| {
//             if use_markdown {
//                 fdisplay(g)
//             } else {
//                 display(g)
//             }
//         })
//         .collect::<Vec<_>>()
//         .join("\n")
// }

/// Turns a slice of GuildIds into a list string with the format `guildName (guildId)\n`
/// use_markdown should be used when displaying them in the discord client
pub async fn display_guild_ids(
    cache_http: impl CacheHttp,
    guild_ids: &[GuildId],
    use_markdown: bool,
) -> anyhow::Result<String> {
    let display_guilds = get_entities(&cache_http, guild_ids)
        .await?
        .iter()
        .map(|g| {
            if use_markdown {
                fdisplay(g)
            } else {
                display(g)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(display_guilds)
}
