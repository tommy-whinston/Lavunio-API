CREATE TABLE mode_events (
    id UUID PRIMARY KEY,
    mode_id UUID NOT NULL REFERENCES modes(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    release_id UUID REFERENCES releases(id) ON DELETE SET NULL,
    event TEXT NOT NULL CHECK (event IN ('download', 'install', 'update')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX mode_events_mode_idx ON mode_events (mode_id, created_at DESC);
CREATE INDEX mode_events_event_idx ON mode_events (event);
