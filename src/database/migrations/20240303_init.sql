CREATE TABLE IF NOT EXISTS admins (
    id VARCHAR(20) NOT NULL, -- discord user id
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    primary key (id)
);

CREATE TABLE IF NOT EXISTS users (
    id VARCHAR(20) NOT NULL, -- discord user id
    servers VARCHAR(20)[] NOT NULL, -- array of server ids
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    primary key (id)
);

CREATE TABLE IF NOT EXISTS bad_actors (
    id SERIAL PRIMARY KEY, -- unique id for the bad actor auto increment
    user_id VARCHAR(20) NOT NULL, -- discord user id
    is_active BOOLEAN NOT NULL DEFAULT TRUE, -- if the bad actor is still active
    actor_type VARCHAR(15) NOT NULL, -- 'spam' | 'impersonation' | 'bigotry'
    originally_created_in VARCHAR(20) NOT NULL, -- server id
    screenshot_proof VARCHAR(50), -- file path to the screenshot
    explanation TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_changed_by VARCHAR(20) NOT NULL
);

CREATE TABLE IF NOT EXISTS server_configs (
    server_id VARCHAR(20) NOT NULL, -- discord server id
    log_channel VARCHAR(20), -- discord channel id
    ping_users BOOLEAN NOT NULL DEFAULT FALSE,
    spam_action_level INT NOT NULL DEFAULT 0,
    impersonation_action_level INT NOT NULL DEFAULT 0,
    bigotry_action_level INT NOT NULL DEFAULT 0,
    timeout_users_with_role BOOLEAN NOT NULL DEFAULT FALSE,
    ignored_roles VARCHAR(20)[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    primary key (server_id)
);