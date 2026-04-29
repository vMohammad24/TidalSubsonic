CREATE TABLE IF NOT EXISTS tidal_tokens (
    tidal_user_id VARCHAR(255) PRIMARY KEY,
    access_token TEXT,
    refresh_token TEXT,
    token_expiry TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_data_request TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS subsonic_users (
    username VARCHAR(255) PRIMARY KEY,
    password TEXT,
    tidal_user_id VARCHAR(255) NOT NULL,
    FOREIGN KEY (tidal_user_id) REFERENCES tidal_tokens (tidal_user_id) ON DELETE CASCADE
);


CREATE TABLE IF NOT EXISTS web_sessions (
    session_id TEXT PRIMARY KEY,
    tidal_user_id TEXT NOT NULL,
    username TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_lastfm_links (
    subsonic_username VARCHAR(255) PRIMARY KEY,
    lastfm_session_key VARCHAR(255) NOT NULL,
    lastfm_username VARCHAR(255) NOT NULL,
    FOREIGN KEY (subsonic_username) REFERENCES subsonic_users (username) ON DELETE CASCADE
);

ALTER TABLE subsonic_users ADD COLUMN IF NOT EXISTS use_playlists BOOLEAN DEFAULT TRUE;
ALTER TABLE subsonic_users ADD COLUMN IF NOT EXISTS use_favorites BOOLEAN DEFAULT TRUE;


CREATE TABLE IF NOT EXISTS play_queues (
    username VARCHAR(255) PRIMARY KEY,
    current_track_id VARCHAR(255),
    position_ms BIGINT,
    track_ids TEXT[] NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (username) REFERENCES subsonic_users(username) ON DELETE CASCADE
);
