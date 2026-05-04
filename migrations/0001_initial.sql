CREATE TABLE IF NOT EXISTS job_executions (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id       TEXT    NOT NULL,
    started_at   TEXT    NOT NULL,  -- ISO 8601 UTC
    finished_at  TEXT    NOT NULL,
    sent         INTEGER NOT NULL DEFAULT 0,
    failed       INTEGER NOT NULL DEFAULT 0,
    errors_json  TEXT
);

CREATE INDEX IF NOT EXISTS idx_executions_job_id ON job_executions (job_id);
CREATE INDEX IF NOT EXISTS idx_executions_started ON job_executions (started_at DESC);

CREATE TABLE IF NOT EXISTS fired_once_jobs (
    job_id    TEXT PRIMARY KEY,
    fired_at  TEXT NOT NULL
);
