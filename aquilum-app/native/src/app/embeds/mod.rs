mod book;
mod cache;
mod dataview;
mod links;
mod media;
mod note;
mod outline;
mod resolver;
#[cfg(test)]
mod testing;

pub use media::{forget_recent, prune_remote};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aquilum_core::Core;
use aquilum_core::search::paths::is_markdown;

use crate::ui::editor::resolve::{BlockList, Book, Fetch, Grid, Media, NoteText, Request, Resolver};
use cache::{Cache, Entry, claim, commit, settle};

struct Inner {
    core: Arc<Core>,
    root: PathBuf,
    document: PathBuf,
    cache: PathBuf,
    wake: Arc<dyn Fn() + Send + Sync>,
    stamps: AtomicU64,
    changed: AtomicBool,
    grids: Cache<Grid>,
    media: Cache<Media>,
    books: Cache<Book>,
    notes: Cache<NoteText>,
    outlines: Cache<Vec<(u8, String)>>,
    block_lists: Cache<BlockList>,
    /// The notes whose text is being drawn around this one, itself included: an embed of any of them would loop.
    chain: Vec<PathBuf>,
    depth: u8,
    /// The embeds of notes shown inside this one, one per note, so their caches survive a re-layout.
    children: Mutex<std::collections::HashMap<PathBuf, Embeds>>,
    parent: Option<std::sync::Weak<Inner>>,
    suggestions: Cache<Vec<String>>,
    links: Mutex<links::Links>,
    book_queue: Mutex<BookQueue>,
    requests: Mutex<Vec<Request>>,
}

#[derive(Default)]
struct BookQueue {
    pending: Vec<(String, u64)>,
    running: bool,
}

impl Inner {
    fn touch(&self) {
        self.changed.store(true, Ordering::Release);
        if let Some(parent) = self.parent.as_ref().and_then(std::sync::Weak::upgrade) {
            parent.touch();
        }
        (self.wake)();
    }

    fn workspace(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    fn source(&self) -> String {
        self.document.to_string_lossy().into_owned()
    }
}

#[derive(Clone)]
pub struct Embeds(Arc<Inner>);

impl Embeds {
    pub fn new(core: Arc<Core>, root: PathBuf, document: PathBuf, cache: PathBuf, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let chain = vec![document.clone()];
        Embeds(Arc::new(Inner {
            core,
            root,
            document,
            cache,
            wake,
            stamps: AtomicU64::new(0),
            changed: AtomicBool::new(false),
            grids: Cache::default(),
            media: Cache::default(),
            books: Cache::default(),
            notes: Cache::default(),
            outlines: Cache::default(),
            block_lists: Cache::default(),
            chain,
            depth: 0,
            children: Mutex::default(),
            parent: None,
            suggestions: Cache::default(),
            links: Mutex::default(),
            book_queue: Mutex::default(),
            requests: Mutex::new(Vec::new()),
        }))
    }

    /// What draws a note shown inside this one: the same vault and wake-up, one note and one level deeper.
    pub fn nested(&self, note: PathBuf) -> Embeds {
        if let Some(known) = self.0.children.lock().unwrap_or_else(|e| e.into_inner()).get(&note) {
            return known.clone();
        }
        let note_key = note.clone();
        let mut chain = self.0.chain.clone();
        chain.push(note.clone());
        let child = Embeds(Arc::new(Inner {
            core: Arc::clone(&self.0.core),
            root: self.0.root.clone(),
            document: note,
            cache: self.0.cache.clone(),
            wake: Arc::clone(&self.0.wake),
            stamps: AtomicU64::new(0),
            changed: AtomicBool::new(false),
            grids: Cache::default(),
            media: Cache::default(),
            books: Cache::default(),
            notes: Cache::default(),
            outlines: Cache::default(),
            block_lists: Cache::default(),
            chain,
            depth: self.0.depth + 1,
            children: Mutex::default(),
            parent: Some(Arc::downgrade(&self.0)),
            suggestions: Cache::default(),
            links: Mutex::default(),
            book_queue: Mutex::default(),
            requests: Mutex::new(Vec::new()),
        }));
        self.0.children.lock().unwrap_or_else(|e| e.into_inner()).insert(note_key, child.clone());
        child
    }

    /// Changes anywhere below this embed: it grows when a nested cache takes a new value.
    pub fn revision(&self) -> u64 {
        let nested: u64 = self.0.children.lock().unwrap_or_else(|e| e.into_inner()).values().map(Embeds::revision).sum();
        self.0.stamps.load(Ordering::Acquire) + nested
    }

    pub fn root(&self) -> &Path {
        &self.0.root
    }

    pub fn document(&self) -> &Path {
        &self.0.document
    }

    pub fn take_requests(&self) -> Vec<Request> {
        std::mem::take(&mut *self.0.requests.lock().unwrap_or_else(|e| e.into_inner()))
    }

    pub fn forget_queries(&self) {
        forget(&self.0);
    }

    pub fn busy(&self) -> bool {
        fn running<T>(cache: &Cache<T>) -> bool {
            cache.entries.lock().unwrap_or_else(|e| e.into_inner()).values().any(|e| e.running)
        }
        running(&self.0.grids) || running(&self.0.media) || running(&self.0.books) || running(&self.0.notes) || running(&self.0.outlines) || running(&self.0.block_lists) || self.0.children.lock().unwrap_or_else(|e| e.into_inner()).values().any(Embeds::busy)
    }

    fn shared(&self, key: &str) -> String {
        format!("{}\0{key}", self.0.document.parent().unwrap_or(&self.0.root).display())
    }

    pub fn forget_paths(&self, paths: &[PathBuf]) {
        media::forget(paths);
        crate::video::forget(paths);
        let changed = |fetch: &Fetch<Media>| matches!(fetch, Fetch::Ready(m) if m.path.as_ref().is_some_and(|p| paths.contains(p)));
        let mut stale = false;
        for entry in self.0.media.entries.lock().unwrap_or_else(|e| e.into_inner()).values_mut() {
            if changed(&entry.value) || entry.next.as_ref().is_some_and(changed) {
                entry.epoch = u64::MAX;
                stale = true;
            }
        }
        if paths.iter().any(|p| is_markdown(p)) {
            self.0.books.epoch.fetch_add(1, Ordering::AcqRel);
            self.0.notes.epoch.fetch_add(1, Ordering::AcqRel);
            self.0.outlines.epoch.fetch_add(1, Ordering::AcqRel);
            self.0.block_lists.epoch.fetch_add(1, Ordering::AcqRel);
            stale = true;
        }
        for child in self.0.children.lock().unwrap_or_else(|e| e.into_inner()).values() {
            child.forget_paths(paths);
        }
        if stale {
            self.0.touch();
        }
    }

    pub fn take_changed(&self) -> bool {
        if !self.0.changed.swap(false, Ordering::AcqRel) {
            return false;
        }
        commit(&self.0.grids, &self.0.stamps);
        commit(&self.0.media, &self.0.stamps);
        commit(&self.0.books, &self.0.stamps);
        commit(&self.0.notes, &self.0.stamps);
        commit(&self.0.outlines, &self.0.stamps);
        commit(&self.0.block_lists, &self.0.stamps);
        for child in self.0.children.lock().unwrap_or_else(|e| e.into_inner()).values() {
            child.take_changed();
        }
        commit(&self.0.suggestions, &self.0.stamps);
        true
    }

    fn fetch<T: Clone + PartialEq + Send + 'static>(&self, cache: fn(&Inner) -> &Cache<T>, key: &str, work: impl FnOnce(&Inner) -> Fetch<T> + Send + 'static) -> (Fetch<T>, u64) {
        let (value, stamp, epoch) = claim(cache(&self.0), key);
        let Some(epoch) = epoch else { return (value, stamp) };
        let inner = Arc::clone(&self.0);
        let key = key.to_owned();
        std::thread::spawn(move || {
            let result = {
                let _zone = aq_trace::zone("background: embed");
                work(&inner)
            };
            if settle(cache(&inner), &key, epoch, result) {
                inner.touch();
            }
        });
        (value, stamp)
    }
}


fn forget(inner: &Arc<Inner>) {
    inner.grids.epoch.fetch_add(1, Ordering::AcqRel);
    inner.suggestions.epoch.fetch_add(1, Ordering::AcqRel);
    inner.books.epoch.fetch_add(1, Ordering::AcqRel);
    inner.notes.epoch.fetch_add(1, Ordering::AcqRel);
    inner.outlines.epoch.fetch_add(1, Ordering::AcqRel);
    inner.block_lists.epoch.fetch_add(1, Ordering::AcqRel);
    for child in inner.children.lock().unwrap_or_else(|e| e.into_inner()).values() {
        forget(&child.0);
    }
    links::refresh(inner);
    inner.touch();
}

fn refresh_after(inner: &Arc<Inner>, query: String, seconds: u32) {
    let weak = Arc::downgrade(inner);
    let epoch = inner.grids.epoch.load(Ordering::Acquire);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(u64::from(seconds.max(1))));
        let Some(inner) = weak.upgrade().filter(|inner| inner.grids.epoch.load(Ordering::Acquire) == epoch) else { return };
        if let Some(entry) = inner.grids.entries.lock().unwrap_or_else(|e| e.into_inner()).get_mut(&query) {
            entry.epoch = u64::MAX;
        }
        Embeds(inner).dataview(&query);
    });
}

