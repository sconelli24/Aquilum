use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use masonry::core::DefaultProperties;
use masonry::peniko::Color;
use masonry::properties::{Collapsible, Gap};
use masonry::widgets::{Flex, ScrollBar};

use aquilum_core::recent::Recent;

use crate::ui::tokens::{self, Builtin, Theme};

static CURRENT: RwLock<Option<Arc<Theme>>> = RwLock::new(None);
static DARK: AtomicBool = AtomicBool::new(false);
/// Индекс показанной темы в `tokens::THEMES`.
static ACTIVE: AtomicUsize = AtomicUsize::new(LIGHT);
static PRIMARY: Mutex<[u8; 3]> = Mutex::new(DEFAULT_PRIMARY);
static CHOICE: RwLock<Option<Arc<Choice>>> = RwLock::new(None);
static GENERATION: AtomicUsize = AtomicUsize::new(0);
static DERIVED: Mutex<Recent<Key, Arc<Theme>>> = Mutex::new(Recent::new(4, usize::MAX));
pub(super) const DEFAULT_PRIMARY: [u8; 3] = [0x14, 0x71, 0xeb];
const LIGHT: usize = 0;
const DARK_THEME: usize = 1;

/// Выбранные темы и правки пользователя поверх них: id темы → токен → «#rrggbb» или число.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Choice {
    pub light: String,
    pub dark: String,
    pub overrides: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Clone, PartialEq)]
struct Key {
    theme: usize,
    primary: [u8; 3],
    overrides: Vec<(String, String)>,
}

thread_local! {
    static PAPER: std::cell::RefCell<Option<Arc<Theme>>> = const { std::cell::RefCell::new(None) };
}

fn printing_now() -> bool {
    PAPER.with_borrow(Option::is_some)
}

pub fn is_dark() -> bool {
    !printing_now() && DARK.load(Ordering::Relaxed)
}

/// Печать — на светлой теме Aquilum, но с размерами и насыщенностью заголовков показанной темы.
pub fn printing<T>(print: impl FnOnce() -> T) -> T {
    let mut paper = *themed(LIGHT, *PRIMARY.lock().unwrap_or_else(|e| e.into_inner()), None);
    let mut shown = *current();
    for index in 0..tokens::NUMBERS.len() {
        if let (Some(from), Some(to)) = (tokens::number_mut(&mut shown, index), tokens::number_mut(&mut paper, index)) {
            *to = *from;
        }
    }
    PAPER.with_borrow_mut(|current| *current = Some(Arc::new(paper)));
    let printed = print();
    PAPER.with_borrow_mut(|current| *current = None);
    printed
}

fn base(index: usize) -> Arc<Theme> {
    static BASE: OnceLock<Vec<Arc<Theme>>> = OnceLock::new();
    let all = BASE.get_or_init(|| tokens::THEMES.iter().map(|builtin| Arc::new(builtin.theme)).collect());
    Arc::clone(&all[index.min(all.len() - 1)])
}

pub fn generation() -> usize {
    GENERATION.load(Ordering::Relaxed)
}

pub fn current() -> Arc<Theme> {
    if let Some(paper) = PAPER.with_borrow(Clone::clone) {
        return paper;
    }
    CURRENT.read().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_else(|| base(LIGHT))
}

/// Показанная тема.
pub fn active() -> &'static Builtin {
    &tokens::THEMES[ACTIVE.load(Ordering::Relaxed).min(tokens::THEMES.len() - 1)]
}

/// Встроенные светлые (`dark == false`) или тёмные темы.
pub fn builtins(dark: bool) -> impl Iterator<Item = &'static Builtin> {
    tokens::THEMES.iter().filter(move |builtin| builtin.dark == dark)
}

pub fn parse_hex(raw: &str) -> Option<[u8; 3]> {
    let hex = raw.trim().strip_prefix('#').unwrap_or(raw.trim());
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

pub fn hex(color: Color) -> String {
    let rgba = color.to_rgba8();
    format!("#{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b)
}

fn rgb(c: [u8; 3]) -> [f32; 4] {
    let [r, g, b] = c.map(|c| f32::from(c) / 255.0);
    [r, g, b, 1.0]
}

/// Тема `index` с акцентом `primary` и правками пользователя; без правок — значения как есть.
fn themed(index: usize, primary: [u8; 3], overrides: Option<&BTreeMap<String, String>>) -> Arc<Theme> {
    let overrides: Vec<(String, String)> = overrides.map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    if primary == DEFAULT_PRIMARY && overrides.is_empty() {
        return base(index);
    }
    let key = Key { theme: index, primary, overrides };
    let mut cache = DERIVED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(theme) = cache.get(&key) {
        return Arc::clone(theme);
    }
    let mut theme = tokens::THEMES[index].theme;
    let mut set = [None; tokens::INPUT_COUNT];
    if primary != DEFAULT_PRIMARY {
        let [r, g, b, a] = rgb(primary);
        set[..3].copy_from_slice(&[Some([r, g, b, a]), Some([r, g, b, a]), Some([r * 0.78, g * 0.78, b * 0.78, a])]);
    }
    for (name, value) in &key.overrides {
        if let Some(at) = tokens::INPUTS.iter().position(|input| input == name)
            && let Some(color) = parse_hex(value)
        {
            set[at] = Some(rgb(color));
        }
    }
    if set.iter().any(Option::is_some) {
        tokens::derive(index, &mut theme, &set);
    }
    for (name, value) in &key.overrides {
        if let Some(at) = tokens::NUMBERS.iter().position(|number| number == name)
            && let (Some(slot), Ok(value)) = (tokens::number_mut(&mut theme, at), value.parse::<f32>())
            && value.is_finite()
        {
            *slot = value;
        }
    }
    let theme = Arc::new(theme);
    cache.put(key, Arc::clone(&theme), 0);
    theme
}

/// Темы для светлого и тёмного режима и правки к ним; применяется следующим `apply_setting`.
pub fn set_choice(choice: Choice) {
    *CHOICE.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(choice));
}

fn pick(dark: bool, choice: Option<&Choice>) -> usize {
    let wanted = choice.map(|c| if dark { c.dark.as_str() } else { c.light.as_str() }).unwrap_or_default();
    tokens::THEMES.iter().position(|builtin| builtin.id == wanted && builtin.dark == dark).unwrap_or(if dark { DARK_THEME } else { LIGHT })
}

pub fn apply_setting(setting: &str, system_dark: bool, primary: &str) {
    let dark = match setting {
        "dark" | "night" => true,
        "light" => false,
        _ => system_dark,
    };
    let rgb = parse_hex(primary).unwrap_or(DEFAULT_PRIMARY);
    *PRIMARY.lock().unwrap_or_else(|e| e.into_inner()) = rgb;
    let choice = CHOICE.read().unwrap_or_else(|e| e.into_inner()).clone();
    let index = pick(dark, choice.as_deref());
    let theme = themed(index, rgb, choice.as_deref().and_then(|c| c.overrides.get(tokens::THEMES[index].id)));
    let mut current = CURRENT.write().unwrap_or_else(|e| e.into_inner());
    if current.as_ref().is_none_or(|old| !Arc::ptr_eq(old, &theme)) {
        GENERATION.fetch_add(1, Ordering::Relaxed);
    }
    *current = Some(theme);
    ACTIVE.store(index, Ordering::Relaxed);
    DARK.store(dark, Ordering::Relaxed);
    crate::system::set_dark_theme(dark);
}

/// Цвет входа `name` (`tokens::INPUTS`) в показанной теме; полупрозрачный — каким он выглядит на фоне заметки.
pub fn input_hex(name: &str) -> Option<String> {
    let at = tokens::INPUTS.iter().position(|input| *input == name)?;
    let t = current();
    let [r, g, b, a] = tokens::input_color(&t, at).components;
    let [br, bg, bb, _] = t.editor_bg.components;
    let over = |c: f32, under: f32| c * a + under * (1.0 - a);
    Some(hex(Color::new([over(r, br), over(g, bg), over(b, bb), 1.0])))
}

/// Число `name` (`tokens::NUMBERS`) в показанной теме.
pub fn number(name: &str) -> Option<f32> {
    let at = tokens::NUMBERS.iter().position(|number| *number == name)?;
    let mut shown = *current();
    tokens::number_mut(&mut shown, at).copied()
}

/// Размер и насыщенность заголовков, цвет жирного и курсива: их смена требует новой раскладки текста.
pub fn typography(t: &Theme) -> [f32; 14] {
    let colored = |c: Color| if c == t.editor_text { 0.0 } else { 1.0 };
    [
        t.editor_heading_1_scale,
        t.editor_heading_2_scale,
        t.editor_heading_3_scale,
        t.editor_heading_4_scale,
        t.editor_heading_5_scale,
        t.editor_heading_6_scale,
        t.editor_heading_1_weight,
        t.editor_heading_2_weight,
        t.editor_heading_3_weight,
        t.editor_heading_4_weight,
        t.editor_heading_5_weight,
        t.editor_heading_6_weight,
        colored(t.editor_strong_color),
        colored(t.editor_em_color),
    ]
}

pub fn default_properties() -> DefaultProperties {
    let mut properties = masonry::theme::default_property_set();
    properties.insert::<Flex, _>(Gap::ZERO);
    properties.insert::<ScrollBar, _>(Collapsible(true));
    crate::ui::style::install(&mut properties, &current());
    properties
}

pub fn background() -> Color {
    current().bg_canvas
}
