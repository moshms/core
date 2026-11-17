ALTER TABLE jobs ADD COLUMN locked_until timestamptz;
CREATE INDEX jobs_running_lease_idx ON jobs (status, locked_until)
    WHERE status = 'running';
