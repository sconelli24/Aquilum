use super::*;
use crate::updater::Attempt;

#[test]
fn app_starts_with_manual_update_popup_after_repeated_failures() {
    let h = Harness::new();
    let data_dir = h.app.workspace.data_dir.clone();

    // Simulate 2 failed update attempts to the new version
    Attempt::record(&data_dir, "99.0.0", "https://example.com/download");
    Attempt::record(&data_dir, "99.0.0", "https://example.com/download");

    // Restart the app with the same data directory
    let mut h = h.restarted(1.0);
    h.app.on_core_events(&mut h.root);
    let _ = h.root.redraw();

    // Check that the dialog offering a manual download opened
    assert!(
        matches!(h.app.popup, Some(Popup::Confirm(_, crate::app::confirm::Confirmed::ManualUpdate(_)))),
        "manual update dialog did not open after failed attempts"
    );

    // Close with Escape
    h.escape();
    assert!(h.app.popup.is_none(), "manual update dialog did not close on Escape");
}

#[test]
fn a_dismissed_update_leaves_only_the_settings_row() {
    let mut h = Harness::new();
    h.app.updater.offer("99.0.0");
    h.app.on_core_events(&mut h.root);
    let close = h.app.update.bar.as_ref().map(|(_, bar)| bar.close).expect("update bar shown");
    h.act(close, crate::ui::widgets::Pressed);
    assert!(h.app.update.bar.is_none(), "bar did not close");
    assert!(h.app.interface.update_dismissed("99.0.0"));
    h.app.rebuild(&mut h.root);
    assert!(h.app.update.bar.is_none(), "dismissed version was offered again");
}
