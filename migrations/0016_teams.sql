CREATE TABLE teams (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    links TEXT NOT NULL DEFAULT '',
    avatar_url TEXT,
    banner_url TEXT,
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'archived')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE team_members (
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'member'
        CHECK (role IN ('owner', 'admin', 'publisher', 'designer', 'member')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, user_id)
);

CREATE INDEX team_members_user_idx ON team_members (user_id);
CREATE INDEX teams_status_idx ON teams (status, created_at);
