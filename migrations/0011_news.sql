CREATE TABLE news (
    id UUID PRIMARY KEY,
    mode_id UUID REFERENCES modes(id) ON DELETE CASCADE,
    author_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'published',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX news_created_idx ON news (created_at DESC);
CREATE INDEX news_mode_idx ON news (mode_id, created_at DESC);
