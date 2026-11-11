CREATE TABLE tag_edits (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    track_ids uuid[] NOT NULL,
    changes jsonb NOT NULL,
    artwork_mime text,
    artwork_data bytea,
    status text NOT NULL DEFAULT 'pending',
    error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX tag_edits_created_idx ON tag_edits (created_at DESC);
