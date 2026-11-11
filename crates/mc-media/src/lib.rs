use std::io::Cursor;
use std::path::Path;

use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFile, TaggedFileExt};
use lofty::picture::{Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::items::Timestamp;
use lofty::tag::{Accessor, ItemKey, Tag};
use mc_core::{CoreError, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct MediaMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i32>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub genres: Vec<String>,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub format: Option<String>,
    pub artwork: Option<ArtworkData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtworkData {
    pub hash: String,
    pub mime: String,
    pub data: Vec<u8>,
}

pub trait MediaParser: Send + Sync {
    fn parse(&self, data: &[u8]) -> Result<MediaMetadata>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LoftyParser;

impl MediaParser for LoftyParser {
    fn parse(&self, data: &[u8]) -> Result<MediaMetadata> {
        let cursor = Cursor::new(data);
        let tagged = Probe::new(cursor)
            .guess_file_type()
            .map_err(|error| CoreError::Validation(format!("unsupported audio: {error}")))?
            .read()
            .map_err(|error| CoreError::Validation(format!("unsupported audio: {error}")))?;

        let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
        let properties = tagged.properties();

        let title = tag
            .and_then(|tag| tag.title())
            .map(|value| value.into_owned())
            .filter(|value| !value.trim().is_empty());
        let artist = tag
            .and_then(|tag| tag.artist())
            .map(|value| value.into_owned())
            .filter(|value| !value.trim().is_empty());
        let album_artist = tag
            .and_then(|tag| tag.get_string(ItemKey::AlbumArtist))
            .map(str::to_owned)
            .filter(|value| !value.trim().is_empty());
        let album = tag
            .and_then(|tag| tag.album())
            .map(|value| value.into_owned())
            .filter(|value| !value.trim().is_empty());
        let year = tag
            .and_then(|tag| tag.date())
            .map(|date| i32::from(date.year));
        let track_no = tag.and_then(|tag| tag.track());
        let disc_no = tag.and_then(|tag| tag.disk());

        let mut genres = tag
            .map(|tag| {
                tag.get_strings(ItemKey::Genre)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        genres.sort_by_key(|genre| genre.to_lowercase());
        genres.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

        let artwork = tag.and_then(|tag| {
            let picture = tag
                .pictures()
                .iter()
                .find(|picture| picture.pic_type() == PictureType::CoverFront)
                .or_else(|| tag.pictures().first())?;

            Some(ArtworkData {
                hash: blake3::hash(picture.data()).to_hex().to_string(),
                mime: picture
                    .mime_type()
                    .map_or("application/octet-stream", |mime| mime.as_str())
                    .to_owned(),
                data: picture.data().to_vec(),
            })
        });

        Ok(MediaMetadata {
            title,
            artist,
            album_artist,
            album,
            year,
            track_no,
            disc_no,
            genres,
            duration_ms: Some(duration_ms(properties.duration())),
            bitrate: properties.audio_bitrate(),
            sample_rate: properties.sample_rate(),
            format: Some(format!("{:?}", tagged.file_type()).to_lowercase()),
            artwork,
        })
    }
}

fn duration_ms(duration: std::time::Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{LoftyParser, MediaParser};

    #[test]
    fn rejects_unknown_data() {
        assert!(LoftyParser.parse(b"not audio").is_err());
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TagChanges {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i32>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub genres: Option<Vec<String>>,
}

impl TagChanges {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.artist.is_none()
            && self.album_artist.is_none()
            && self.album.is_none()
            && self.year.is_none()
            && self.track_no.is_none()
            && self.disc_no.is_none()
            && self.genres.is_none()
    }
}

pub fn write_tags_to_path(path: &Path, changes: &TagChanges, artwork: Option<&[u8]>) -> Result<()> {
    let mut tagged = lofty::read_from_path(path)
        .map_err(|error| CoreError::Validation(format!("read audio tags: {error}")))?;

    apply_changes(&mut tagged, changes, artwork)?;

    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|error| {
            CoreError::Unavailable(format!("write audio tags {}: {error}", path.display()))
        })
}

pub fn write_tags_to_bytes(
    data: Vec<u8>,
    changes: &TagChanges,
    artwork: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(data);
    let mut tagged = Probe::new(&mut cursor)
        .guess_file_type()
        .map_err(|error| CoreError::Validation(format!("read audio tags: {error}")))?
        .read()
        .map_err(|error| CoreError::Validation(format!("read audio tags: {error}")))?;

    apply_changes(&mut tagged, changes, artwork)?;

    tagged
        .save_to(&mut cursor, WriteOptions::default())
        .map_err(|error| CoreError::Unavailable(format!("write audio tags: {error}")))?;

    Ok(cursor.into_inner())
}

fn apply_changes(
    tagged: &mut TaggedFile,
    changes: &TagChanges,
    artwork: Option<&[u8]>,
) -> Result<()> {
    if tagged.primary_tag().is_none() && tagged.first_tag().is_none() {
        let tag_type = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tag_type));
    }

    let tag = if let Some(tag) = tagged.primary_tag_mut() {
        tag
    } else if let Some(tag) = tagged.first_tag_mut() {
        tag
    } else {
        return Err(CoreError::Validation(
            "audio file has no writable tag".to_owned(),
        ));
    };

    if let Some(title) = &changes.title {
        tag.set_title(title.clone());
    }

    if let Some(artist) = &changes.artist {
        tag.set_artist(artist.clone());
    }

    if let Some(album_artist) = &changes.album_artist {
        tag.insert_text(ItemKey::AlbumArtist, album_artist.clone());
    }

    if let Some(album) = &changes.album {
        tag.set_album(album.clone());
    }

    if let Some(year) = changes.year {
        if year > 0 {
            tag.set_date(Timestamp {
                year: u16::try_from(year).unwrap_or(0),
                ..Timestamp::default()
            });
        } else {
            tag.remove_date();
        }
    }

    if let Some(track_no) = changes.track_no {
        if track_no > 0 {
            tag.set_track(track_no);
        } else {
            tag.remove_track();
        }
    }

    if let Some(disc_no) = changes.disc_no {
        if disc_no > 0 {
            tag.set_disk(disc_no);
        } else {
            tag.remove_disk();
        }
    }

    if let Some(genres) = &changes.genres {
        let value = genres.join("; ");
        if value.is_empty() {
            tag.remove_genre();
        } else {
            tag.set_genre(value);
        }
    }

    if let Some(artwork) = artwork {
        let mut picture =
            Picture::from_reader(&mut Cursor::new(artwork.to_vec())).unwrap_or_else(|_| {
                Picture::unchecked(artwork.to_vec())
                    .pic_type(PictureType::CoverFront)
                    .build()
            });
        picture.set_pic_type(PictureType::CoverFront);

        if tag.pictures().is_empty() {
            tag.push_picture(picture);
        } else {
            tag.set_picture(0, picture);
        }
    }

    Ok(())
}

#[cfg(test)]
mod write_tests {
    use super::{write_tags_to_bytes, LoftyParser, MediaParser, TagChanges};

    #[test]
    fn writes_tags_to_bytes() {
        let data = include_bytes!("../tests/fixtures/sample.mp3").to_vec();
        let changes = TagChanges {
            title: Some("Rewritten Song".to_owned()),
            artist: Some("Rewritten Artist".to_owned()),
            album_artist: Some("Rewritten Album Artist".to_owned()),
            album: Some("Rewritten Album".to_owned()),
            year: Some(2030),
            track_no: Some(7),
            disc_no: Some(2),
            genres: Some(vec!["Jazz".to_owned(), "Fusion".to_owned()]),
        };

        let updated = write_tags_to_bytes(data, &changes, None).unwrap();
        let metadata = LoftyParser.parse(&updated).unwrap();

        assert_eq!(metadata.title.as_deref(), Some("Rewritten Song"));
        assert_eq!(metadata.artist.as_deref(), Some("Rewritten Artist"));
        assert_eq!(
            metadata.album_artist.as_deref(),
            Some("Rewritten Album Artist")
        );
        assert_eq!(metadata.album.as_deref(), Some("Rewritten Album"));
        assert_eq!(metadata.year, Some(2030));
        assert_eq!(metadata.track_no, Some(7));
        assert_eq!(metadata.disc_no, Some(2));
        assert!(metadata.genres.iter().any(|genre| genre.contains("Jazz")));
    }
}
