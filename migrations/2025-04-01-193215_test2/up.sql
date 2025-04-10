-- Your SQL goes here
CREATE TABLE mounts (
    owned_by TEXT NOT NULL,
    mount_name TEXT NOT NULL,
    img_path TEXT NOT NULL,
    mnt_path TEXT NOT NULL,
    max_bytes BIGINT NOT NULL,
    date_created TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    date_accessed TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    locked BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (owned_by, mount_name)
);