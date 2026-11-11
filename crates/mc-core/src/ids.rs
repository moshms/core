use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_type {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
            #[serde(transparent)]
            pub struct $name(Uuid);

            impl $name {
                #[must_use]
                pub fn new() -> Self {
                    Self(Uuid::now_v7())
                }

                #[must_use]
                pub const fn from_uuid(value: Uuid) -> Self {
                    Self(value)
                }

                #[must_use]
                pub const fn as_uuid(&self) -> &Uuid {
                    &self.0
                }

                #[must_use]
                pub const fn into_uuid(self) -> Uuid {
                    self.0
                }
            }

            impl Default for $name {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.0.fmt(f)
                }
            }

            impl FromStr for $name {
                type Err = uuid::Error;

                fn from_str(value: &str) -> Result<Self, Self::Err> {
                    Uuid::parse_str(value).map(Self)
                }
            }

            impl From<Uuid> for $name {
                fn from(value: Uuid) -> Self {
                    Self(value)
                }
            }

            impl From<$name> for Uuid {
                fn from(value: $name) -> Self {
                    value.0
                }
            }
        )+
    };
}

id_type!(
    StorageId,
    LibraryId,
    MediaFileId,
    TrackId,
    AlbumId,
    ArtistId,
    GenreId,
    ArtworkId,
    UserId,
    SessionId,
    ApiTokenId,
    PlaylistId,
    PlaylistEntryId,
    UploadId,
    ImportJobId,
    ImportItemId,
    TagEditId,
    JobId,
    ScanRunId,
    TranscodeProfileId,
    ShareId,
);

#[cfg(test)]
mod tests {
    use super::TrackId;

    #[test]
    fn id_roundtrips_through_text() {
        let id = TrackId::new();
        let parsed = id.to_string().parse::<TrackId>().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn id_serializes_as_string() {
        let id = TrackId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
    }
}
