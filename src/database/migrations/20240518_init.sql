CREATE TABLE IF NOT EXISTS admins (
    user_id VARCHAR(20) NOT NULL PRIMARY KEY,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS users (
    user_id VARCHAR(20) NOT NULL PRIMARY KEY,
    guild_ids VARCHAR(20)[] NOT NULL,
    user_type VARCHAR(8) NOT NULL, -- 'reporter' or 'listener'
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS server_configs (
    guild_id VARCHAR(20) NOT NULL PRIMARY KEY,
    log_channel_id VARCHAR(20),
    ping_users BOOLEAN NOT NULL DEFAULT FALSE,
    ping_role VARCHAR(20),
    spam_action_level INT NOT NULL DEFAULT 0,
    impersonation_action_level INT NOT NULL DEFAULT 0,
    bigotry_action_level INT NOT NULL DEFAULT 0,
    ignored_roles VARCHAR(20)[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS bad_actors (
    id SERIAL PRIMARY KEY,
    user_id VARCHAR(20) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    actor_type VARCHAR(15) NOT NULL, -- 'spam' or 'impersonation' or 'bigotry'
    origin_guild_id VARCHAR(20) NOT NULL,
    screenshot_proof VARCHAR(50),
    explanation TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_by_user_id VARCHAR(20) NOT NULL
);

CREATE TABLE IF NOT EXISTS user_scores (
    user_id VARCHAR(20) PRIMARY KEY,
    score INT NOT NULL
);

CREATE TABLE IF NOT EXISTS guild_scores (
    guild_id VARCHAR(20) PRIMARY KEY,
    score INT NOT NULL
);

CREATE TABLE IF NOT EXISTS webhooks (
    guild_id VARCHAR(20) NOT NULL PRIMARY KEY,
    guild_name VARCHAR(50) NOT NULL,
    webhook_url VARCHAR(200) NOT NULL
);