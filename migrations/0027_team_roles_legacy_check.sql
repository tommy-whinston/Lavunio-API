-- Роли команд ушли в team_roles (0024): имена ролей теперь произвольные.
-- Легаси-колонки team_members.role и team_invitations.role синхронизируются
-- API при переименовании ролей, но CHECK с фиксированным набором имён
-- ломает любые кастомные роли (500 на update_role и назначении участнику).
ALTER TABLE team_members DROP CONSTRAINT IF EXISTS team_members_role_check;
ALTER TABLE team_invitations DROP CONSTRAINT IF EXISTS team_invitations_role_check;
