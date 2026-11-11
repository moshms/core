CREATE TABLE storages (
    id uuid PRIMARY KEY,
    name text NOT NULL,
    kind text NOT NULL,
    config jsonb NOT NULL DEFAULT '{}'::jsonb,
    access text NOT NULL DEFAULT 'rw' CHECK (access IN ('ro', 'rw')),
    status text NOT NULL DEFAULT 'online',
    last_scan_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE libraries (
    id uuid PRIMARY KEY,
    name text NOT NULL UNIQUE,
    storage_id uuid NOT NULL REFERENCES storages(id) ON DELETE CASCADE,
    root_prefix text NOT NULL DEFAULT '',
    path_template text,
    is_import_target boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE artworks (
    id uuid PRIMARY KEY,
    hash text NOT NULL UNIQUE,
    source text NOT NULL,
    mime text NOT NULL,
    size_bytes bigint NOT NULL,
    width integer,
    height integer,
    data bytea NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE artists (
    id uuid PRIMARY KEY,
    name text NOT NULL,
    sort_name text,
    mbid text,
    artwork_id uuid REFERENCES artworks(id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX artists_name_unique ON artists (lower(name));

CREATE TABLE albums (
    id uuid PRIMARY KEY,
    title text NOT NULL,
    album_artist_id uuid NOT NULL REFERENCES artists(id),
    year integer NOT NULL DEFAULT 0,
    release_type text,
    mbid text,
    artwork_id uuid REFERENCES artworks(id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX albums_identity_unique ON albums (album_artist_id, lower(title), year);

CREATE TABLE media_files (
    id uuid PRIMARY KEY,
    library_id uuid NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    key text NOT NULL,
    size bigint NOT NULL,
    etag text NOT NULL,
    mtime_ms bigint NOT NULL,
    content_hash text,
    format text,
    codec text,
    bitrate integer,
    sample_rate integer,
    duration_ms bigint,
    scan_state text NOT NULL DEFAULT 'indexed',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (library_id, key)
);

CREATE INDEX media_files_library_state_idx ON media_files (library_id, scan_state);

CREATE TABLE tracks (
    id uuid PRIMARY KEY,
    title text NOT NULL,
    album_id uuid REFERENCES albums(id) ON DELETE SET NULL,
    disc_no integer NOT NULL DEFAULT 0,
    track_no integer NOT NULL DEFAULT 0,
    year integer NOT NULL DEFAULT 0,
    mbid text,
    isrc text,
    lyrics text,
    duration_ms bigint,
    primary_file_id uuid NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (primary_file_id)
);

CREATE INDEX tracks_album_idx ON tracks (album_id, disc_no, track_no);

CREATE TABLE track_artists (
    track_id uuid NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    artist_id uuid NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    role text NOT NULL DEFAULT 'main',
    position integer NOT NULL DEFAULT 0,
    PRIMARY KEY (track_id, artist_id, role)
);

CREATE TABLE genres (
    id uuid PRIMARY KEY,
    name text NOT NULL
);

CREATE UNIQUE INDEX genres_name_unique ON genres (lower(name));

CREATE TABLE track_genres (
    track_id uuid NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    genre_id uuid NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
    PRIMARY KEY (track_id, genre_id)
);

CREATE TABLE scan_runs (
    id uuid PRIMARY KEY,
    library_id uuid NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    mode text NOT NULL,
    status text NOT NULL DEFAULT 'running',
    counters jsonb NOT NULL DEFAULT '{}'::jsonb,
    error text,
    started_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);

CREATE INDEX scan_runs_library_idx ON scan_runs (library_id, started_at DESC);
