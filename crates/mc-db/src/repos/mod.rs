pub mod albums;
pub mod annotations;
pub mod api_tokens;
pub mod artists;
pub mod artworks;
pub mod audit;
pub mod catalog;
pub mod genres;
pub mod libraries;
pub mod media_files;
pub mod permissions;
pub mod play_queue;
pub mod playlists;
pub mod roles;
pub mod scan_runs;
pub mod sessions;
pub mod storages;
pub mod tag_edits;
pub mod tracks;
pub mod uploads;
pub mod users;

use mc_core::CoreError;

pub(crate) fn db_error(context: &str, error: sqlx::Error) -> CoreError {
    CoreError::Internal(format!("{context}: {error}"))
}

pub(crate) fn db_error_unique(context: &str, error: sqlx::Error) -> CoreError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23505") {
            return CoreError::Conflict(format!("{context}: already exists"));
        }
    }

    db_error(context, error)
}
