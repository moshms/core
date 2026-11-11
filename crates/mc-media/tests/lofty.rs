use mc_media::{LoftyParser, MediaParser};

#[test]
fn reads_tags_and_embedded_artwork() {
    let metadata = LoftyParser
        .parse(include_bytes!("fixtures/sample.mp3"))
        .unwrap();

    assert_eq!(metadata.title.as_deref(), Some("Test Song"));
    assert_eq!(metadata.artist.as_deref(), Some("Test Artist"));
    assert_eq!(metadata.album_artist.as_deref(), Some("Test Artist"));
    assert_eq!(metadata.album.as_deref(), Some("Test Album"));
    assert_eq!(metadata.year, Some(2024));
    assert_eq!(metadata.track_no, Some(1));
    assert_eq!(metadata.genres, vec!["Rock".to_owned()]);
    assert_eq!(metadata.format.as_deref(), Some("mpeg"));
    assert!(metadata.duration_ms.unwrap() >= 900);

    let artwork = metadata.artwork.unwrap();
    assert_eq!(artwork.mime, "image/png");
    assert!(!artwork.data.is_empty());
    assert_eq!(artwork.hash.len(), 64);
}
