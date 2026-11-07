ALTER TABLE users
    ADD COLUMN minecraft_uuid UUID NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN skin_url TEXT,
    ADD COLUMN skin_model TEXT NOT NULL DEFAULT 'classic'
        CHECK (skin_model IN ('classic', 'slim')),
    ADD COLUMN cape_url TEXT,
    ADD COLUMN skin_updated_at TIMESTAMPTZ;

CREATE UNIQUE INDEX users_minecraft_uuid_idx ON users (minecraft_uuid);

CREATE TABLE game_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    access_hash TEXT NOT NULL UNIQUE,
    client_hash TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX game_sessions_user_idx ON game_sessions (user_id);

CREATE TABLE game_joins (
    server_id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (server_id, user_id)
);
