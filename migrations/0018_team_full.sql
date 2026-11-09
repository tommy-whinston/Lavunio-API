ALTER TABLE teams
    ADD COLUMN join_mode TEXT NOT NULL DEFAULT 'invite'
        CHECK (join_mode IN ('invite', 'request', 'open')),
    ADD COLUMN verified BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE modes
    ADD CONSTRAINT modes_team_id_fkey
    FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE SET NULL;

CREATE TABLE team_join_requests (
    id UUID PRIMARY KEY,
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'accepted', 'rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    resolved_by UUID REFERENCES users(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX team_join_requests_pending_idx
    ON team_join_requests (team_id, user_id)
    WHERE status = 'pending';
CREATE INDEX team_join_requests_team_idx
    ON team_join_requests (team_id, status, created_at);

CREATE TABLE team_followers (
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, user_id)
);

CREATE INDEX team_followers_user_idx ON team_followers (user_id);

CREATE TABLE team_news (
    id UUID PRIMARY KEY,
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    author_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'published'
        CHECK (status IN ('published', 'hidden')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX team_news_team_idx ON team_news (team_id, created_at DESC);
