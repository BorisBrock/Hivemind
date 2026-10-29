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
