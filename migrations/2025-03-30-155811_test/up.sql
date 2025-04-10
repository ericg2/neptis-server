-- Your SQL goes here
CREATE DOMAIN EncodedHashType AS TEXT CHECK (LENGTH(VALUE) > 0);

CREATE TABLE users (
    user_name TEXT PRIMARY KEY,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    password_hash EncodedHashType NOT NULL, -- Change to TEXT
    create_date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    max_data_bytes BIGINT NOT NULL,
    max_snapshot_bytes BIGINT NOT NULL
);

-- Create the sessions table
CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    user_name TEXT REFERENCES users(user_name) ON DELETE CASCADE,
    create_date TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expire_date TIMESTAMP NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE
);