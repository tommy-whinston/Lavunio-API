CREATE TABLE users (
    id UUID PRIMARY KEY,
    ely_id TEXT UNIQUE,
    username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    avatar_url TEXT,
    role TEXT NOT NULL DEFAULT 'author'
        CHECK (role IN ('player', 'author', 'moderator', 'admin')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE teams (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE team_members (
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'member',
    PRIMARY KEY (team_id, user_id)
);

CREATE TABLE modes (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    short_description TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'pending', 'published', 'rejected', 'hidden')),
    cover_url TEXT,
    author_id UUID NOT NULL REFERENCES users(id),
    team_id UUID REFERENCES teams(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE mode_authors (
    mode_id UUID NOT NULL REFERENCES modes(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'author',
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mode_id, user_id)
);

CREATE TABLE tags (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL
);

CREATE TABLE mode_tags (
    mode_id UUID NOT NULL REFERENCES modes(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (mode_id, tag_id)
);

CREATE TABLE releases (
    id UUID PRIMARY KEY,
    mode_id UUID NOT NULL REFERENCES modes(id) ON DELETE CASCADE,
    version TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'pending', 'published', 'rejected', 'hidden', 'superseded')),
    channel TEXT NOT NULL DEFAULT 'stable'
        CHECK (channel IN ('stable', 'beta')),
    changelog TEXT NOT NULL DEFAULT '',
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (mode_id, version)
);

CREATE TABLE release_assets (
    id UUID PRIMARY KEY,
    release_id UUID NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    kind TEXT NOT NULL
        CHECK (kind IN ('archive', 'cover', 'manifest', 'screenshot', 'other')),
    url TEXT NOT NULL DEFAULT '',
    size BIGINT NOT NULL DEFAULT 0,
    sha256 TEXT,
    parts JSONB NOT NULL DEFAULT '[]'::jsonb,
    manifest JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE reviews (
    id UUID PRIMARY KEY,
    mode_id UUID NOT NULL REFERENCES modes(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    rating SMALLINT NOT NULL CHECK (rating BETWEEN 1 AND 5),
    text TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'published', 'rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (mode_id, user_id)
);

CREATE TABLE reports (
    id UUID PRIMARY KEY,
    target_type TEXT NOT NULL CHECK (target_type IN ('mode', 'release', 'review', 'user')),
    target_id UUID NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'in_review', 'closed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE moderation_actions (
    id UUID PRIMARY KEY,
    moderator_id UUID NOT NULL REFERENCES users(id),
    target_type TEXT NOT NULL,
    target_id UUID NOT NULL,
    action TEXT NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE library (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mode_id UUID NOT NULL REFERENCES modes(id) ON DELETE CASCADE,
    favorite BOOLEAN NOT NULL DEFAULT false,
    hidden BOOLEAN NOT NULL DEFAULT false,
    added_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, mode_id)
);

CREATE TABLE downloads (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    release_id UUID NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'downloading', 'paused', 'done', 'error')),
    bytes_done BIGINT NOT NULL DEFAULT 0,
    bytes_total BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE installs (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mode_id UUID NOT NULL REFERENCES modes(id) ON DELETE CASCADE,
    release_id UUID REFERENCES releases(id) ON DELETE SET NULL,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_played_at TIMESTAMPTZ,
    playtime_secs BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE follows (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    author_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, author_id)
);

CREATE TABLE api_tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    scopes JSONB NOT NULL DEFAULT '[]'::jsonb,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX modes_status_updated_idx ON modes (status, updated_at DESC);
CREATE INDEX modes_author_idx ON modes (author_id);
CREATE INDEX releases_mode_status_idx ON releases (mode_id, status);
CREATE INDEX release_assets_release_idx ON release_assets (release_id);
CREATE INDEX reviews_mode_status_idx ON reviews (mode_id, status);
CREATE INDEX reports_status_idx ON reports (status, created_at);
CREATE INDEX api_tokens_hash_idx ON api_tokens (token_hash);
CREATE INDEX installs_user_idx ON installs (user_id);
