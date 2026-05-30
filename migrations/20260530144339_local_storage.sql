CREATE TABLE IF NOT EXISTS local_playlists (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(255) NOT NULL,
    name TEXT NOT NULL,
    comment TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (username) REFERENCES subsonic_users (username) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS local_playlist_tracks (
    playlist_id UUID NOT NULL,
    track_id VARCHAR(255) NOT NULL,
    position INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (playlist_id, position),
    FOREIGN KEY (playlist_id) REFERENCES local_playlists (id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_local_playlist_tracks_playlist_id ON local_playlist_tracks (playlist_id);

CREATE TABLE IF NOT EXISTS local_favorite_tracks (
    username VARCHAR(255) NOT NULL,
    track_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (username, track_id),
    FOREIGN KEY (username) REFERENCES subsonic_users (username) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS local_favorite_albums (
    username VARCHAR(255) NOT NULL,
    album_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (username, album_id),
    FOREIGN KEY (username) REFERENCES subsonic_users (username) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS local_favorite_artists (
    username VARCHAR(255) NOT NULL,
    artist_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (username, artist_id),
    FOREIGN KEY (username) REFERENCES subsonic_users (username) ON DELETE CASCADE
);
