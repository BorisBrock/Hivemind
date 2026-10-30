-- Named groups of projects in the sidebar.
CREATE TABLE project_groups (
    id         INTEGER PRIMARY KEY,
    name       TEXT    NOT NULL,
    collapsed  INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL,
    created_at TEXT    NOT NULL
);

-- Deleting a group keeps its projects; they become ungrouped.
ALTER TABLE projects ADD COLUMN group_id INTEGER REFERENCES project_groups (id) ON DELETE SET NULL;
