use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Uploader,
    Listener,
}

impl Role {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Uploader => "uploader",
            Self::Listener => "listener",
        }
    }

    #[must_use]
    pub const fn can_upload(self) -> bool {
        matches!(self, Self::Admin | Self::Uploader)
    }

    #[must_use]
    pub const fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown role: {0}")]
pub struct ParseRoleError(String);

impl FromStr for Role {
    type Err = ParseRoleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "admin" => Ok(Self::Admin),
            "uploader" => Ok(Self::Uploader),
            "listener" => Ok(Self::Listener),
            other => Err(ParseRoleError(other.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Role;

    #[test]
    fn role_roundtrips_through_text() {
        for role in [Role::Admin, Role::Uploader, Role::Listener] {
            assert_eq!(role.to_string().parse::<Role>().unwrap(), role);
        }
    }
}
