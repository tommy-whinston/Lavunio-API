ALTER TABLE team_members
    DROP CONSTRAINT team_members_role_check;
ALTER TABLE team_members
    ADD CONSTRAINT team_members_role_check
    CHECK (role IN ('owner', 'admin', 'publisher', 'designer', 'builder', 'modeler', 'member'));

ALTER TABLE team_members
    ADD COLUMN permissions JSONB NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE team_invitations
    DROP CONSTRAINT team_invitations_role_check;
ALTER TABLE team_invitations
    ADD CONSTRAINT team_invitations_role_check
    CHECK (role IN ('admin', 'publisher', 'designer', 'builder', 'modeler', 'member'));
