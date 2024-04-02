use std::{num::NonZeroU64, str::FromStr};

pub fn parse_snowflake(snowflake: impl Into<String>) -> anyhow::Result<std::num::NonZeroU64> {
    NonZeroU64::from_str(snowflake.into().as_str()).map_err(|err| anyhow::Error::new(err))
}

#[cfg(test)]
mod tests {
    use super::parse_snowflake;

    #[test]
    fn parse_snowflake_works() {
        let parsed = parse_snowflake("471026181211422721");

        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().get(), 471026181211422721);
    }

    #[test]
    fn parse_snowflake_fails() {
        let parsed = parse_snowflake("0");

        assert!(parsed.is_err());
    }
}
