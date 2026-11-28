-- Легаси-CHECK может отсутствовать на свежих БД (0016 его не создаёт),
-- поэтому дропаем аккуратно, а расширяем список ролей как раньше.
DO $$ BEGIN
    ALTER TABLE team_members
        DROP CONSTRAINT team_members_role_check;
EXCEPTION WHEN undefined_object THEN NULL; END $$;
ALTER TABLE team_members
    ADD CONSTRAINT team_members_role_check
    CHECK (role IN ('owner', 'admin', 'publisher', 'designer', 'builder', 'modeler', 'member'));

ALTER TABLE team_members
    ADD COLUMN permissions JSONB NOT NULL DEFAULT '{}'::jsonb;

DO $$ BEGIN
    ALTER TABLE team_invitations
        DROP CONSTRAINT team_invitations_role_check;
EXCEPTION WHEN undefined_object THEN NULL; END $$;
ALTER TABLE team_invitations
    ADD CONSTRAINT team_invitations_role_check
    CHECK (role IN ('admin', 'publisher', 'designer', 'builder', 'modeler', 'member'));
