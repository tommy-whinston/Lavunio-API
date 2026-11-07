ALTER TABLE release_assets
    ADD COLUMN file_name TEXT,
    ADD COLUMN storage_key TEXT,
    ADD COLUMN storage_kind TEXT;

CREATE INDEX release_assets_storage_key_idx ON release_assets (storage_key);
