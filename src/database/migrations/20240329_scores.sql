CREATE TABLE IF NOT EXISTS user_scores (
    discord_id VARCHAR(20) PRIMARY KEY,
    score INT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS guild_scores (
    guild_id VARCHAR(20) PRIMARY KEY,
    score INT NOT NULL DEFAULT 0
);