use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Copy, Debug)]
pub enum ItemType {
    Track,
    Album,
    Artist,
}

impl ItemType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Album => "album",
            Self::Artist => "artist",
        }
    }
}

pub async fn star(pool: &PgPool, user_id: Uuid, item_type: ItemType, item_id: Uuid) -> Result<()> {
    sqlx::query(
        "INSERT INTO annotations (user_id, item_type, item_id, starred_at)
         VALUES ($1, $2, $3, now())
         ON CONFLICT (user_id, item_type, item_id) DO UPDATE SET starred_at = now()",
    )
    .bind(user_id)
    .bind(item_type.as_str())
    .bind(item_id)
    .execute(pool)
    .await
    .map_err(|error| db_error("star item", error))?;

    Ok(())
}

pub async fn unstar(
    pool: &PgPool,
    user_id: Uuid,
    item_type: ItemType,
    item_id: Uuid,
) -> Result<()> {
    sqlx::query(
        "UPDATE annotations
         SET starred_at = NULL
         WHERE user_id = $1 AND item_type = $2 AND item_id = $3",
    )
    .bind(user_id)
    .bind(item_type.as_str())
    .bind(item_id)
    .execute(pool)
    .await
    .map_err(|error| db_error("unstar item", error))?;

    Ok(())
}

pub async fn scrobble(
    pool: &PgPool,
    user_id: Uuid,
    track_id: Uuid,
    submission: bool,
) -> Result<()> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| db_error("begin scrobble transaction", error))?;

    if submission {
        sqlx::query("INSERT INTO play_events (id, user_id, track_id) VALUES ($1, $2, $3)")
            .bind(Uuid::now_v7())
            .bind(user_id)
            .bind(track_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| db_error("insert play event", error))?;
    }

    let play_increment = i32::from(submission);

    sqlx::query(
        "INSERT INTO annotations (user_id, item_type, item_id, play_count, last_played_at)
         VALUES ($1, 'track', $2, $3, now())
         ON CONFLICT (user_id, item_type, item_id) DO UPDATE SET
             play_count = annotations.play_count + EXCLUDED.play_count,
             last_played_at = now()",
    )
    .bind(user_id)
    .bind(track_id)
    .bind(play_increment)
    .execute(&mut *transaction)
    .await
    .map_err(|error| db_error("update play annotation", error))?;

    transaction
        .commit()
        .await
        .map_err(|error| db_error("commit scrobble transaction", error))
}
