use super::*;

#[test]
fn reset_graph_settings_shows_confirmation_dialog_and_restores_defaults() {
    let mut h = Harness::new();
    let vault = h.vault.path().to_path_buf();
    std::fs::write(vault.join("a.md"), "[[b]]").unwrap();
    std::fs::write(vault.join("b.md"), "[[a]]").unwrap();
    h.app.tree.paths_changed(&mut h.root, &[vault.join("a.md"), vault.join("b.md")]);
    h.app.open_graph(&mut h.root);
    assert!(h.wait_for(|h| h.app.graph_loaded().is_some()), "graph did not load");

    let custom_prefs = crate::interface::GraphPrefs {
        node_size: 2.5,
        spread: 0.5,
        highlight_depth: 3,
        labels: false,
        heatmap_axis: 2,
        panel: true,
    };
    h.app.interface.set_graph(custom_prefs);
    h.app.set_graph_share(0.75);
    h.app.show_graph(&mut h.root);

    let reset_button = h.app.graph_reset_button().expect("settings reset button");

    h.act(reset_button, crate::ui::widgets::Pressed);
    assert!(h.app.reset_graph_dialog_layer().is_some(), "reset confirmation dialog did not open");

    let dialog_layer = h.app.reset_graph_dialog_layer().expect("dialog layer");
    h.act(dialog_layer, crate::ui::dialog::DialogAnswer::Cancel);
    assert!(!h.app.reset_graph_dialog_layer().is_some(), "dialog did not close after cancel");
    assert_eq!(h.app.interface.graph(), custom_prefs, "settings were reset on cancel");
    assert_eq!(h.app.graph_share(), 0.75, "time slider was reset on cancel");

    h.act(reset_button, crate::ui::widgets::Pressed);
    assert!(h.app.reset_graph_dialog_layer().is_some());
    h.escape();
    assert!(!h.app.reset_graph_dialog_layer().is_some(), "dialog did not close on Escape");
    assert_eq!(h.app.interface.graph(), custom_prefs);

    h.act(reset_button, crate::ui::widgets::Pressed);
    assert!(h.app.reset_graph_dialog_layer().is_some());
    let dialog_layer = h.app.reset_graph_dialog_layer().expect("dialog layer");
    h.act(dialog_layer, crate::ui::dialog::DialogAnswer::Confirm);
    assert!(!h.app.reset_graph_dialog_layer().is_some(), "dialog stayed open after confirmation");
    assert_eq!(h.app.interface.graph(), crate::interface::GraphPrefs::default(), "settings were not reset to defaults");
    assert_eq!(h.app.graph_share(), 0.0, "time filter was not reset to 0");
}
