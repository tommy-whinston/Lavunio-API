INSERT INTO teams (id, slug, title, description, owner_id, verified, join_mode)
SELECT gen_random_uuid(), 'tm', 'TM', 'Команда Lavunio', id, true, 'invite'
FROM users
WHERE username = 'tm'
ON CONFLICT (slug) DO NOTHING;

INSERT INTO team_members (team_id, user_id, role)
SELECT t.id, u.id, 'owner'
FROM teams t
JOIN users u ON u.username = 'tm'
WHERE t.slug = 'tm'
ON CONFLICT DO NOTHING;

UPDATE modes m
SET team_id = t.id
FROM teams t
WHERE t.slug = 'tm'
  AND m.team_id IS NULL
  AND m.author_id = (SELECT id FROM users WHERE username = 'tm');
