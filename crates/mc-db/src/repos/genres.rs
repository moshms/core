use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

pub async fn set_for_track(pool: &PgPool, track_id: Uuid, names: &[String]) -> Result<()> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| db_error("begin track genres transaction", error))?;

    sqlx::query("DELETE FROM track_genres WHERE track_id = $1")
        .bind(track_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| db_error("clear track genres", error))?;

    for name in names {
        let genre_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO genres (id, name)
             VALUES ($1, $2)
             ON CONFLICT (lower(name)) DO UPDATE SET name = genres.name
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(name)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| db_error("upsert genre", error))?;

        sqlx::query(
            "INSERT INTO track_genres (track_id, genre_id)
             VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(track_id)
        .bind(genre_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| db_error("insert track genre", error))?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| db_error("commit track genres transaction", error))
}
