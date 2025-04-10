-- Your SQL goes here
CREATE TABLE repo_jobs (
    id UUID PRIMARY KEY,
    snapshot_id TEXT,
    point_owned_by TEXT NOT NULL,
    point_name TEXT NOT NULL,
    job_type SMALLINT NOT NULL,
    job_status SMALLINT NOT NULL,
    used_bytes BIGINT NOT NULL,
    total_bytes BIGINT,
    errors TEXT[] NOT NULL,
    create_date TIMESTAMP NOT NULL,
    end_date TIMESTAMP
);