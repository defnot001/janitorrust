ALTER TABLE users ADD COLUMN IF NOT EXISTS user_type VARCHAR(8) NOT NULL DEFAULT 'reporter'; -- reporter | listener

ALTER TABLE server_configs ADD COLUMN IF NOT EXISTS ping_role VARCHAR(20);