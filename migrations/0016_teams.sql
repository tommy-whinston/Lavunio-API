-- Команды. Файл должен одинаково работать на свежей БД и на БД, где
-- teams/team_members уже созданы ранними миграциями (0001 создаёт их
-- legacy-версии), поэтому схема добивается через IF NOT EXISTS и ALTERы.
-- Легаси-CHECK на team_members.role снимает миграция 0027.

CREATE TABLE IF NOT EXISTS teams (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    links TEXT NOT NULL DEFAULT '',
    avatar_url TEXT,
    banner_url TEXT,
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'archived')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Колонки, которых нет в legacy-версии из 0001.
ALTER TABLE teams ADD COLUMN IF NOT EXISTS title TEXT NOT NULL DEFAULT '';
ALTER TABLE teams ADD COLUMN IF NOT EXISTS links TEXT NOT NULL DEFAULT '';
ALTER TABLE teams ADD COLUMN IF NOT EXISTS banner_url TEXT;
ALTER TABLE teams ADD COLUMN IF NOT EXISTS owner_id UUID REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE teams ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'active';
ALTER TABLE teams ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();

DO $$ BEGIN
    ALTER TABLE teams ALTER COLUMN name DROP NOT NULL;
EXCEPTION WHEN undefined_column THEN NULL; END $$;

CREATE TABLE IF NOT EXISTS team_members (
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'member',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (team_id, user_id)
);

ALTER TABLE team_members ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT now();

CREATE INDEX IF NOT EXISTS team_members_user_idx ON team_members (user_id);
CREATE INDEX IF NOT EXISTS teams_status_idx ON teams (status, created_at);
