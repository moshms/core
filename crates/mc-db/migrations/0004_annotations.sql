CREATE TABLE annotations (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_type text NOT NULL CHECK (item_type IN ('track', 'album', 'artist')),
    item_id uuid NOT NULL,
    starred_at timestamptz,
    rating integer,
    play_count integer NOT NULL DEFAULT 0,
    last_played_at timestamptz,
    PRIMARY KEY (user_id, item_type, item_id)
);

CREATE INDEX annotations_item_idx ON annotations (item_type, item_id);
CREATE INDEX annotations_starred_idx ON annotations (user_id, starred_at DESC)
    WHERE starred_at IS NOT NULL;

CREATE TABLE playlists (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name text NOT NULL,
    is_public boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX playlists_owner_idx ON playlists (owner_id);

CREATE TABLE playlist_entries (
    playlist_id uuid NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id uuid NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position integer NOT NULL,
    PRIMARY KEY (playlist_id, position)
);

CREATE INDEX playlist_entries_track_idx ON playlist_entries (track_id);

CREATE TABLE play_queues (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    track_ids jsonb NOT NULL DEFAULT '[]'::jsonb,
    current_index integer NOT NULL DEFAULT 0,
    position_ms bigint NOT NULL DEFAULT 0,
    changed_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE play_events (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    track_id uuid NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    played_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX play_events_user_idx ON play_events (user_id, played_at DESC);
