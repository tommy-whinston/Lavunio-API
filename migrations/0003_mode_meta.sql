ALTER TABLE modes
    ADD COLUMN mc_version TEXT NOT NULL DEFAULT '',
    ADD COLUMN loader TEXT NOT NULL DEFAULT 'vanilla',
    ADD COLUMN loader_version TEXT,
    ADD COLUMN size_label TEXT;
