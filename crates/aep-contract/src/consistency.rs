//! Read-your-writes, without assuming how a backend is built.
//!
//! A conformance suite cannot sleep. If it could, it would be testing the machine it runs on rather
//! than the implementation, and the first slow CI box would turn a correct backend red.
//!
//! So every accepted mutation returns an opaque [`ConsistencyToken`], and a query may demand a view
//! no older than that token. An immediately consistent backend satisfies it for free; a projected
//! one blocks until its projection catches up. Neither has to say which it is.

use std::fmt;

use aep_domain::error::ParseError;

/// An opaque marker for a point in a backend's history.
///
/// Nothing outside the backend that issued it may interpret the contents. It is passed back, not
/// read.
#[derive(
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct ConsistencyToken(String);

impl ConsistencyToken {
    /// Wraps a backend-issued token.
    pub fn new(value: impl Into<String>) -> Result<Self, ParseError> {
        let value: String = value.into();
        if value.is_empty() {
            return Err(ParseError::identifier(
                "consistency token",
                &value,
                "must not be empty".to_owned(),
            ));
        }
        if value.len() > 512 {
            return Err(ParseError::identifier(
                "consistency token",
                &value,
                "must be at most 512 characters".to_owned(),
            ));
        }
        Ok(Self(value))
    }

    /// The token as a string slice, for a backend reading its own token back.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConsistencyToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for ConsistencyToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConsistencyToken({})", self.0)
    }
}

/// How fresh a read has to be.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "consistency", rename_all = "snake_case")]
pub enum QueryConsistency {
    /// Whatever the backend has now. The default, and the cheap one.
    #[default]
    Current,
    /// A view no older than the successful command that produced this token.
    AtLeast {
        /// The token that mutation returned.
        token: ConsistencyToken,
    },
}

impl QueryConsistency {
    /// Demands a view at least as fresh as `token`.
    pub fn at_least(token: ConsistencyToken) -> Self {
        Self::AtLeast { token }
    }

    /// The token this read waits for, if any.
    pub fn token(&self) -> Option<&ConsistencyToken> {
        match self {
            Self::Current => None,
            Self::AtLeast { token } => Some(token),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_carried_not_interpreted() {
        let token = ConsistencyToken::new("seq:41").expect("valid");
        assert_eq!(token.as_str(), "seq:41");
        assert!(ConsistencyToken::new("").is_err());
    }

    #[test]
    fn current_is_the_default_and_waits_for_nothing() {
        assert_eq!(QueryConsistency::default(), QueryConsistency::Current);
        assert!(QueryConsistency::default().token().is_none());

        let token = ConsistencyToken::new("seq:41").expect("valid");
        let demanded = QueryConsistency::at_least(token.clone());
        assert_eq!(demanded.token(), Some(&token));
    }

    #[test]
    fn consistency_round_trips_through_serde() {
        let demanded = QueryConsistency::at_least(ConsistencyToken::new("seq:7").expect("valid"));
        let json = serde_json::to_value(&demanded).expect("serialises");
        assert_eq!(json["consistency"], "at_least");
        assert_eq!(json["token"], "seq:7");
        let parsed: QueryConsistency = serde_json::from_value(json).expect("deserialises");
        assert_eq!(parsed, demanded);
    }
}
