CREATE TABLE uploads (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    library_id uuid NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    filename text NOT NULL,
    size bigint NOT NULL,
    upload_offset bigint NOT NULL DEFAULT 0,
    staging_key text NOT NULL,
    status text NOT NULL DEFAULT 'in_progress',
    error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX uploads_user_idx ON uploads (user_id, created_at DESC);
CREATE INDEX uploads_status_idx ON uploads (status);
