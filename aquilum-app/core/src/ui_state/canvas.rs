//! Last camera of each canvas file. Obsidian keeps no viewport in the file, so it lives here.
use super::database::UiStateDatabase;
use super::error::UiStateError;
use super::models::CanvasView;
use rusqlite::{params, OptionalExtension};

impl UiStateDatabase {
    pub fn load_canvas_view(&self, workspace_id: uuid::Uuid, canvas_file: &str) -> Result<Option<CanvasView>, UiStateError> {
        Ok(self
            .connection
            .query_row(
                "SELECT center_x, center_y, scale FROM canvas_views
                 WHERE workspace_id = ?1 AND canvas_file = ?2",
                params![workspace_id.to_string(), canvas_file],
                |row| Ok(CanvasView { center_x: row.get(0)?, center_y: row.get(1)?, scale: row.get(2)? }),
            )
            .optional()?)
    }

    pub fn save_canvas_view(
        &self,
        workspace_id: uuid::Uuid,
        canvas_file: &str,
        view: CanvasView,
        now_ms: i64,
    ) -> Result<(), UiStateError> {
        let finite = view.center_x.is_finite() && view.center_y.is_finite() && view.scale.is_finite();
        if !finite || view.scale <= 0.0 {
            return Err(UiStateError::InvalidInput { message: "canvas view must be finite with a positive scale".to_owned() });
        }
        self.connection.execute(
            "INSERT INTO canvas_views(workspace_id, canvas_file, center_x, center_y, scale, updated_at_ms)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(workspace_id, canvas_file) DO UPDATE SET
                 center_x = excluded.center_x,
                 center_y = excluded.center_y,
                 scale = excluded.scale,
                 updated_at_ms = excluded.updated_at_ms",
            params![workspace_id.to_string(), canvas_file, view.center_x, view.center_y, view.scale, now_ms],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_view_round_trips_and_rejects_bad_values() {
        let mut database = UiStateDatabase::memory().expect("database");
        let workspace_id = database.resolve_workspace("C:/notes", 1).expect("workspace");
        assert!(database.load_canvas_view(workspace_id, "Board.canvas").expect("load").is_none());
        let view = CanvasView { center_x: 10.5, center_y: -3.0, scale: 0.75 };
        database.save_canvas_view(workspace_id, "Board.canvas", view, 5).expect("save");
        assert_eq!(database.load_canvas_view(workspace_id, "Board.canvas").expect("load"), Some(view));
        let bad = CanvasView { scale: 0.0, ..view };
        assert!(database.save_canvas_view(workspace_id, "Board.canvas", bad, 6).is_err());
    }
}
