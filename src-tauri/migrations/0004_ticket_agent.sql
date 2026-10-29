-- Which agent owns `agent_session_id`; a session can only be continued by the same agent.
ALTER TABLE tickets ADD COLUMN agent TEXT;
