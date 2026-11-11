use std::fmt;
use std::str::FromStr;

use mc_core::{CoreError, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Key(String);

impl Key {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate(&value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn root() -> Self {
        Self(String::new())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn join(&self, segment: &str) -> Result<Self> {
        if self.is_root() {
            Self::new(segment)
        } else {
            Self::new(format!("{}/{}", self.0, segment))
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Key {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl FromStr for Key {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

fn validate(value: &str) -> Result<()> {
    if value.is_empty() {
        return Ok(());
    }

    if value.starts_with('/') || value.ends_with('/') {
        return Err(CoreError::Validation(format!(
            "storage key must not start or end with '/': {value:?}"
        )));
    }

    if value.contains('\\') || value.contains('\0') {
        return Err(CoreError::Validation(format!(
            "storage key contains forbidden characters: {value:?}"
        )));
    }

    for segment in value.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(CoreError::Validation(format!(
                "invalid storage key segment: {value:?}"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Key;

    #[test]
    fn accepts_relative_keys() {
        assert!(Key::new("artist/album/01 track.flac").is_ok());
        assert!(Key::new("").is_ok());
    }

    #[test]
    fn rejects_traversal_and_absolute_keys() {
        for value in ["../x", "a/../../b", "/x", "x/", "a//b", "a\\b", "a/./b"] {
            assert!(Key::new(value).is_err(), "{value:?} should be rejected");
        }
    }
}
