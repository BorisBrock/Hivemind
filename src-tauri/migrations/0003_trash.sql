-- Closed Done tickets go to the trash: hidden from the board, kept with their transcript
-- until restored or deleted for good. NULL for tickets on the board.
ALTER TABLE tickets ADD COLUMN archived_at TEXT;

CREATE INDEX tickets_by_archived ON tickets (archived_at);
