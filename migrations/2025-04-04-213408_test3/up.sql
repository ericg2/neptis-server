-- Your SQL goes here
DROP TABLE IF EXISTS mounts;

CREATE TABLE mounts (
    owned_by TEXT NOT NULL,
    mount_name TEXT NOT NULL,
    data_img_path TEXT NOT NULL,
    data_mnt_path TEXT NOT NULL,
    repo_password TEXT NOT NULL,
    data_max_bytes BIGINT NOT NULL,
    repo_img_path TEXT NOT NULL,
    repo_mnt_path TEXT NOT NULL,
    repo_max_bytes BIGINT NOT NULL,
    date_created TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    data_accessed TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    repo_accessed TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    locked BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (owned_by, mount_name)
);