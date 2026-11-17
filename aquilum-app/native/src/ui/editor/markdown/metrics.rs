use masonry::core::BrushIndex;
use masonry::parley::{FontFamily, Layout};

pub const LIST_START: f64 = 12.0;
pub const LIST_LEVEL: f64 = 24.0;
pub const QUOTE_BAR: f64 = 3.0;
pub const QUOTE_INSET: f64 = 2.0;
pub const BULLET: f64 = 5.0;
pub(super) const INLINE_CODE_PADDING: f32 = 4.0;
pub(super) const TAG_PADDING: f32 = 6.0;
pub(super) const COLLAPSED: f32 = 0.001;
pub const CHECKBOX: f32 = 20.0;
pub(super) const CHECKBOX_RADIUS: f64 = 6.0;
pub(super) const BOX_CHARS: usize = 3;
pub(super) const TAB_SIZE: f32 = 4.0;
pub(super) const SCRIPT: f32 = 0.75;

pub struct Fonts {
    pub size: f32,
    pub mono: FontFamily<'static>,
    pub ch: f32,
    pub mark: f32,
    pub space: f32,
    pub line: f32,
    pub digits: Vec<Layout<BrushIndex>>,
    pub callouts: bool,
    pub tags: bool,
    pub typography: Typography,
}

/// Оформление текста из темы, от которого зависит раскладка.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typography {
    /// Размер заголовка уровня `i + 1` — доля размера текста.
    pub scale: [f32; 6],
    pub weight: [f32; 6],
    /// Жирный и курсив красятся своим цветом.
    pub strong: bool,
    pub emphasis: bool,
}

impl Typography {
    pub fn current() -> Typography {
        let [s1, s2, s3, s4, s5, s6, w1, w2, w3, w4, w5, w6, strong, emphasis] = crate::ui::theme::typography(&crate::ui::theme::current());
        Typography { scale: [s1, s2, s3, s4, s5, s6], weight: [w1, w2, w3, w4, w5, w6], strong: strong > 0.0, emphasis: emphasis > 0.0 }
    }
}

impl Fonts {
    pub fn new(size: f32, callouts: bool, tags: bool) -> Fonts {
        let mono = crate::ui::theme::editor_font("iA Writer Mono");
        Fonts { size, mono, ch: 0.0, mark: 0.0, space: 0.0, line: 0.0, digits: Vec::new(), callouts, tags, typography: Typography::current() }
    }
}
