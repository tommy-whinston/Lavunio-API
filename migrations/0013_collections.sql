CREATE TABLE collections (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'published'
        CHECK (status IN ('published', 'hidden')),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE collection_modes (
    collection_id UUID NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    mode_id UUID NOT NULL REFERENCES modes(id) ON DELETE CASCADE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (collection_id, mode_id)
);

CREATE INDEX collections_sort_idx ON collections (sort_order, created_at DESC);
CREATE INDEX collection_modes_collection_idx ON collection_modes (collection_id, sort_order);
