ALTER TABLE IF EXISTS server_configs ADD COLUMN honeypot_channel_id VARCHAR(20);
ALTER TABLE IF EXISTS server_configs ADD COLUMN honeypot_action_level INT NOT NULL DEFAULT 0;
