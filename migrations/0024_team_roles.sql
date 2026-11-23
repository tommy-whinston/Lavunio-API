-- Настраиваемые роли команды.
--
-- Старое поле team_members.role остаётся как есть: код продолжает работать,
-- пока API не перейдёт на role_id. Миграция аддитивная — существующие данные
-- только копируются в team_roles и связываются через role_id.

CREATE TABLE team_roles (
    id UUID PRIMARY KEY,
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    permissions JSONB NOT NULL DEFAULT '{}'::jsonb,
    position INTEGER NOT NULL DEFAULT 0,
    is_default BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (team_id, name)
);

CREATE INDEX team_roles_team_idx ON team_roles (team_id, position);

ALTER TABLE team_members ADD COLUMN role_id UUID REFERENCES team_roles(id) ON DELETE SET NULL;
ALTER TABLE team_invitations ADD COLUMN role_id UUID REFERENCES team_roles(id) ON DELETE SET NULL;

-- Роли, которые уже используются участниками, становятся ролями команды.
INSERT INTO team_roles (id, team_id, name, permissions, position, is_default)
SELECT
    gen_random_uuid(),
    used.team_id,
    used.role,
    CASE used.role
        WHEN 'owner' THEN '{"manage_team":true,"manage_members":true,"manage_modes":true,"manage_news":true}'::jsonb
        WHEN 'admin' THEN '{"manage_team":true,"manage_members":true,"manage_modes":false,"manage_news":true}'::jsonb
        WHEN 'publisher' THEN '{"manage_team":false,"manage_members":false,"manage_modes":true,"manage_news":false}'::jsonb
        WHEN 'designer' THEN '{"manage_team":false,"manage_members":false,"manage_modes":true,"manage_news":false}'::jsonb
        WHEN 'builder' THEN '{"manage_team":false,"manage_members":false,"manage_modes":true,"manage_news":false}'::jsonb
        WHEN 'modeler' THEN '{"manage_team":false,"manage_members":false,"manage_modes":true,"manage_news":false}'::jsonb
        ELSE '{}'::jsonb
    END,
    CASE used.role
        WHEN 'owner' THEN 0
        WHEN 'admin' THEN 10
        WHEN 'publisher' THEN 20
        WHEN 'designer' THEN 30
        WHEN 'builder' THEN 40
        WHEN 'modeler' THEN 50
        ELSE 60
    END,
    used.role = 'owner'
FROM (SELECT DISTINCT team_id, role FROM team_members) AS used
ON CONFLICT (team_id, name) DO NOTHING;

UPDATE team_members m
SET role_id = r.id
FROM team_roles r
WHERE r.team_id = m.team_id
  AND r.name = m.role
  AND m.role_id IS NULL;

UPDATE team_invitations i
SET role_id = r.id
FROM team_roles r
WHERE r.team_id = i.team_id
  AND r.name = i.role
  AND i.role_id IS NULL;
