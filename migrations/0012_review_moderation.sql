ALTER TABLE reviews
    ADD COLUMN author_reply TEXT,
    ADD COLUMN author_reply_at TIMESTAMPTZ;

CREATE INDEX reviews_status_created_idx ON reviews (status, created_at DESC);
