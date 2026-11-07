ALTER TABLE releases
    ADD COLUMN rejection_note TEXT,
    ADD COLUMN moderated_at TIMESTAMPTZ,
    ADD COLUMN moderated_by UUID REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX releases_status_idx ON releases (status);
CREATE INDEX moderation_actions_created_idx ON moderation_actions (created_at DESC);
CREATE INDEX moderation_actions_target_idx ON moderation_actions (target_type, target_id);
