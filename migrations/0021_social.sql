ALTER TABLE users
    ADD COLUMN last_seen_at TIMESTAMPTZ;

CREATE TABLE friend_requests (
    id UUID PRIMARY KEY,
    requester_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    addressee_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'accepted', 'declined')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    responded_at TIMESTAMPTZ,
    CHECK (requester_id <> addressee_id)
);

CREATE UNIQUE INDEX friend_requests_pending_idx
    ON friend_requests (requester_id, addressee_id)
    WHERE status = 'pending';
CREATE INDEX friend_requests_addressee_idx
    ON friend_requests (addressee_id, status, created_at);
CREATE INDEX friend_requests_requester_idx
    ON friend_requests (requester_id, status, created_at);

CREATE TABLE notifications (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    link_type TEXT,
    link_id TEXT,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX notifications_user_idx
    ON notifications (user_id, read_at, created_at DESC);

CREATE TABLE mode_comments (
    id UUID PRIMARY KEY,
    mode_id UUID NOT NULL REFERENCES modes(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_id UUID REFERENCES mode_comments(id) ON DELETE CASCADE,
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'published'
        CHECK (status IN ('published', 'hidden')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX mode_comments_mode_idx
    ON mode_comments (mode_id, created_at);
CREATE INDEX mode_comments_user_idx
    ON mode_comments (user_id, created_at);

CREATE TABLE chat_messages (
    id UUID PRIMARY KEY,
    sender_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_id UUID REFERENCES users(id) ON DELETE CASCADE,
    team_id UUID REFERENCES teams(id) ON DELETE CASCADE,
    body TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((recipient_id IS NOT NULL) <> (team_id IS NOT NULL))
);

CREATE INDEX chat_direct_idx
    ON chat_messages (sender_id, recipient_id, created_at);
CREATE INDEX chat_team_idx
    ON chat_messages (team_id, created_at);
