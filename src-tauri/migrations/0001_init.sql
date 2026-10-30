CREATE TABLE projects (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL,
    path        TEXT    NOT NULL,
    -- Index into the active colour palette, so switching palettes recolours projects.
    color_index INTEGER NOT NULL,
    hidden      INTEGER NOT NULL DEFAULT 0,
    sort_order  INTEGER NOT NULL,
    created_at  TEXT    NOT NULL
);

CREATE TABLE tickets (
    id               INTEGER PRIMARY KEY,
    project_id       INTEGER NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    title            TEXT    NOT NULL,
    body             TEXT    NOT NULL,
    status           TEXT    NOT NULL
        CHECK (status IN ('planning', 'ready', 'in_progress', 'needs_input', 'done')),
    -- Rank within its column; fractional so a move only rewrites one row.
    position         REAL    NOT NULL,
    agent_session_id TEXT,
    -- Which agent owns `agent_session_id`; a session can only be continued by the same agent.
    agent            TEXT,
    -- Prompt for the agent's next run when it continues an existing session
    -- (a user reply, or a comment when a Done ticket is sent back to Ready).
    pending_prompt   TEXT,
    -- Parallel tickets (questions, analysis) run alongside the project's sequential queue.
    parallel         BOOLEAN NOT NULL DEFAULT 0,
    created_at       TEXT    NOT NULL,
    updated_at       TEXT    NOT NULL
);

CREATE INDEX tickets_by_column ON tickets (status, position);
CREATE INDEX tickets_by_project ON tickets (project_id);

-- Append-only agent transcript of a ticket.
CREATE TABLE ticket_events (
    id         INTEGER PRIMARY KEY,
    ticket_id  INTEGER NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    seq        INTEGER NOT NULL,
    kind       TEXT    NOT NULL,
    payload    TEXT    NOT NULL,
    created_at TEXT    NOT NULL,
    UNIQUE (ticket_id, seq)
);

-- One row per finished agent turn, for usage statistics. Kept when a ticket is deleted.
CREATE TABLE agent_turns (
    id            INTEGER PRIMARY KEY,
    ticket_id     INTEGER REFERENCES tickets (id) ON DELETE SET NULL,
    created_at    TEXT    NOT NULL,
    input_tokens  INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    cost_usd      REAL    NOT NULL
);

CREATE INDEX agent_turns_by_time ON agent_turns (created_at);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
