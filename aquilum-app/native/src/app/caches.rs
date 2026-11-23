use masonry::app::RenderRoot;

use super::App;
use super::graph_page::GraphState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moment {
    Hide,
    Tab,
    Vault,
}

use Moment::{Hide, Tab, Vault};

struct Cache {
    name: &'static str,
    when: &'static [Moment],
    clear: fn(&mut App, &mut RenderRoot),
}

const CATALOG: &[Cache] = &[
    Cache { name: "images", when: &[Hide, Tab, Vault], clear: |_, _| crate::images::trim() },
    Cache { name: "recent embeds", when: &[Hide, Vault], clear: |_, _| super::embeds::forget_recent() },
    Cache { name: "cover patterns", when: &[Hide, Vault], clear: |_, _| crate::cover::trim() },
    Cache { name: "city cover tile", when: &[Hide, Vault], clear: |_, _| crate::cover::forget_tile() },
    Cache { name: "book cover rays and star", when: &[Hide, Vault], clear: |_, _| crate::ui::book::forget_art() },
    Cache { name: "render buffers and glyph atlas", when: &[Hide], clear: |_, _| crate::render::trim_soon() },
    Cache { name: "glyph atlas after canvas", when: &[Tab], clear: |_, _| crate::render::trim_soon() },
    Cache { name: "graph frames and labels", when: &[Hide], clear: |app, root| app.drop_graph_frames(root) },
    Cache { name: "PDF frames and caches", when: &[Hide, Vault], clear: |app, root| app.drop_reader_frames(root) },
    Cache { name: "links and analysis panel", when: &[Tab, Vault], clear: |app, _| (app.listing, app.panel_body, app.panel_key) = (None, None, None) },
    Cache { name: "history in panel", when: &[Tab, Vault], clear: |app, _| (app.history, app.history_body) = (None, None) },
    Cache { name: "version preview", when: &[Tab, Vault], clear: |app, root| app.drop_version(root) },
    Cache { name: "versions of closed tabs", when: &[Tab], clear: |app, _| {
        let tabs = &app.tabs;
        app.opened_versions.retain(|id, _| tabs.tabs().iter().any(|tab| tab.id == *id));
    } },
    Cache { name: "graph", when: &[Tab], clear: |app, root| app.release_graph(root) },
    Cache { name: "link graph for analysis", when: &[Tab, Vault], clear: |app, _| app.workspace.core.search.forget_analysis() },
    Cache { name: "Wikixiv", when: &[Tab, Vault], clear: |app, _| app.workspace.core.wikixiv.forget() },
    Cache { name: "vault graph", when: &[Vault], clear: |app, _| app.graph = GraphState::default() },
    Cache { name: "open canvas", when: &[Vault], clear: |app, root| app.leave_canvas(root) },
    Cache { name: "open versions", when: &[Vault], clear: |app, _| {
        app.opened_versions.clear();
        app.version_notice = None;
    } },
    Cache { name: "home page", when: &[Vault], clear: |app, _| app.home = None },
    Cache { name: "vault attachments", when: &[Vault], clear: |_, _| aquilum_core::files::attachments::forget_attachments() },
    Cache { name: "released heap memory", when: &[Hide, Tab, Vault], clear: |_, _| crate::system::release_memory() },
];

impl App {
    pub(crate) fn forget(&mut self, moment: Moment, root: &mut RenderRoot) {
        for cache in CATALOG.iter().filter(|cache| cache.when.contains(&moment)) {
            let _zone = aq_trace::zone(cache.name);
            (cache.clear)(self, root);
        }
    }
}
