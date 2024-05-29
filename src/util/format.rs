use std::borrow::Cow;

use poise::serenity_prelude as serenity;
use serenity::{PartialGuild, User, UserId};

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

pub fn user_mention(user: &UserId) -> String {
    format!("<@{}>", user)
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
