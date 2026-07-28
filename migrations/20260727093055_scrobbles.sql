CREATE TABLE IF NOT EXISTS scrobbles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(255) NOT NULL,
    track_id VARCHAR(255) NOT NULL,
    played_at TIMESTAMPTZ NOT NULL,
    submission BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (username) REFERENCES subsonic_users (username) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_scrobbles_username_played_at ON scrobbles (username, played_at DESC);
CREATE INDEX IF NOT EXISTS idx_scrobbles_track_id ON scrobbles (track_id);
CREATE INDEX IF NOT EXISTS idx_scrobbles_played_at ON scrobbles (played_at DESC);
