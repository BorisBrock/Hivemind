-- Prompt for the agent's next run when it continues an existing session
-- (a user reply, or a comment when a Done ticket is sent back to Ready).
ALTER TABLE tickets ADD COLUMN pending_prompt TEXT;
