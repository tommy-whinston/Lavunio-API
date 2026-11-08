CREATE TABLE wishlist (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mode_id UUID NOT NULL REFERENCES modes(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, mode_id)
);

CREATE INDEX wishlist_mode_idx ON wishlist (mode_id);
