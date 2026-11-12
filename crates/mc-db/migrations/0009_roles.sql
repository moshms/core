CREATE TABLE roles (
    id uuid PRIMARY KEY,
    name text NOT NULL,
    color integer,
    position integer NOT NULL,
    permissions bigint NOT NULL DEFAULT 0,
    is_everyone boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX roles_everyone_unique ON roles (is_everyone) WHERE is_everyone;
CREATE UNIQUE INDEX roles_name_unique ON roles (lower(name));

CREATE TABLE user_roles (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id uuid NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, role_id)
);

CREATE INDEX user_roles_role_idx ON user_roles (role_id);

CREATE TABLE permission_overwrites (
    resource_type text NOT NULL CHECK (resource_type IN ('library', 'playlist')),
    resource_id uuid NOT NULL,
    target_type text NOT NULL CHECK (target_type IN ('role', 'user')),
    target_id uuid NOT NULL,
    allow bigint NOT NULL DEFAULT 0,
    deny bigint NOT NULL DEFAULT 0,
    CHECK (allow & deny = 0),
    PRIMARY KEY (resource_type, resource_id, target_type, target_id)
);

CREATE INDEX overwrites_target_idx ON permission_overwrites (target_type, target_id);

CREATE TABLE audit_log (
    id uuid PRIMARY KEY,
    actor_id uuid REFERENCES users(id) ON DELETE SET NULL,
    action text NOT NULL,
    target_type text NOT NULL,
    target_id uuid,
    changes jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX audit_log_created_idx ON audit_log (created_at DESC);

ALTER TABLE users ADD COLUMN perm_version bigint NOT NULL DEFAULT 0;
ALTER TABLE media_files ADD COLUMN uploaded_by uuid REFERENCES users(id) ON DELETE SET NULL;

INSERT INTO roles (id, name, color, position, permissions, is_everyone)
VALUES
    (gen_random_uuid(), '@everyone', NULL, 0,
     (1::bigint << 15) | (1::bigint << 23) | (1::bigint << 24), true),
    (gen_random_uuid(), 'Администратор', 15158332, 100, 1::bigint, false),
    (gen_random_uuid(), 'Загрузчик', 3447003, 50, (1::bigint << 12), false);

INSERT INTO user_roles (user_id, role_id)
SELECT u.id, r.id
FROM users u
JOIN roles r ON r.name = 'Администратор'
WHERE u.role = 'admin'
ON CONFLICT DO NOTHING;

INSERT INTO user_roles (user_id, role_id)
SELECT u.id, r.id
FROM users u
JOIN roles r ON r.name = 'Загрузчик'
WHERE u.role = 'uploader'
ON CONFLICT DO NOTHING;

INSERT INTO permission_overwrites (resource_type, resource_id, target_type, target_id, allow, deny)
SELECT 'library', l.id, 'role', r.id,
       (1::bigint << 10) | (1::bigint << 11), 0
FROM libraries l
CROSS JOIN roles r
WHERE r.is_everyone
  AND NOT EXISTS (SELECT 1 FROM user_libraries ul WHERE ul.library_id = l.id)
ON CONFLICT DO NOTHING;

INSERT INTO permission_overwrites (resource_type, resource_id, target_type, target_id, allow, deny)
SELECT 'library', l.id, 'role', r.id, 0, (1::bigint << 10)
FROM libraries l
CROSS JOIN roles r
WHERE r.is_everyone
  AND EXISTS (SELECT 1 FROM user_libraries ul WHERE ul.library_id = l.id)
ON CONFLICT DO NOTHING;

INSERT INTO permission_overwrites (resource_type, resource_id, target_type, target_id, allow, deny)
SELECT 'library', ul.library_id, 'user', ul.user_id, (1::bigint << 10), 0
FROM user_libraries ul
ON CONFLICT DO NOTHING;

INSERT INTO permission_overwrites (resource_type, resource_id, target_type, target_id, allow, deny)
SELECT 'playlist', p.id, 'role', r.id, (1::bigint << 20), 0
FROM playlists p
CROSS JOIN roles r
WHERE r.is_everyone AND p.is_public
ON CONFLICT DO NOTHING;

CREATE OR REPLACE FUNCTION cleanup_permission_overwrites()
RETURNS trigger AS $$
BEGIN
    IF TG_TABLE_NAME = 'libraries' THEN
        DELETE FROM permission_overwrites
        WHERE resource_type = 'library' AND resource_id = OLD.id;
    ELSIF TG_TABLE_NAME = 'playlists' THEN
        DELETE FROM permission_overwrites
        WHERE resource_type = 'playlist' AND resource_id = OLD.id;
    ELSIF TG_TABLE_NAME = 'roles' THEN
        DELETE FROM permission_overwrites
        WHERE target_type = 'role' AND target_id = OLD.id;
    ELSIF TG_TABLE_NAME = 'users' THEN
        DELETE FROM permission_overwrites
        WHERE target_type = 'user' AND target_id = OLD.id;
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER libraries_cleanup_overwrites
AFTER DELETE ON libraries
FOR EACH ROW EXECUTE FUNCTION cleanup_permission_overwrites();

CREATE TRIGGER playlists_cleanup_overwrites
AFTER DELETE ON playlists
FOR EACH ROW EXECUTE FUNCTION cleanup_permission_overwrites();

CREATE TRIGGER roles_cleanup_overwrites
AFTER DELETE ON roles
FOR EACH ROW EXECUTE FUNCTION cleanup_permission_overwrites();

CREATE TRIGGER users_cleanup_overwrites
AFTER DELETE ON users
FOR EACH ROW EXECUTE FUNCTION cleanup_permission_overwrites();
