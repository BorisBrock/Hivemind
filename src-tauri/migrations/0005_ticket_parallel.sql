-- Parallel tickets (questions, analysis) run alongside the project's sequential queue.
ALTER TABLE tickets ADD COLUMN parallel BOOLEAN NOT NULL DEFAULT 0;
