use super::super::error::UiStateError;
use rusqlite::Connection;

pub fn migrate_v9_to_v10(connection: &Connection) -> Result<(), UiStateError> {
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE IF NOT EXISTS canvas_views (
             workspace_id TEXT NOT NULL,
             canvas_file TEXT NOT NULL,
             center_x REAL NOT NULL,
             center_y REAL NOT NULL,
             scale REAL NOT NULL,
             updated_at_ms INTEGER NOT NULL,
             PRIMARY KEY(workspace_id, canvas_file),
             FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
         );
         PRAGMA user_version = 10;
         COMMIT;",
    )?;
    Ok(())
}
