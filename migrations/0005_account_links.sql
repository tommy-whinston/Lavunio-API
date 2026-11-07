CREATE TABLE link_codes (
    code TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE skin_profiles (
    id UUID PRIMARY KEY,
    identity_id UUID NOT NULL UNIQUE REFERENCES identities(id) ON DELETE CASCADE,
    username TEXT NOT NULL,
    uuid TEXT NOT NULL,
    skin_url TEXT,
    cape_url TEXT,
    model TEXT NOT NULL DEFAULT 'classic'
        CHECK (model IN ('classic', 'slim')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX link_codes_user_idx ON link_codes (user_id);
