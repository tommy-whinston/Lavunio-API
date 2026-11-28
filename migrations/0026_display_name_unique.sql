CREATE UNIQUE INDEX IF NOT EXISTS users_display_name_lower_idx
    ON users (lower(display_name));
