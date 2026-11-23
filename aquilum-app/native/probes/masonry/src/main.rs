//! Masonry probe (step 2 of stage 2): the `masonry_core` widget core on its own host:
//! winit 0.31 + vello_cpu + softbuffer, without `masonry_winit` (it has only GPU backends and winit 0.30).
//!
//! The layout is the same as in `aquilum-native`: a sidebar and a text column, only the text here is
//! an editable `TextArea` as an editor stub. What is checked: does Masonry fit on our host without
//! a fork, do focus, input, IME and the clipboard work, and how much memory it costs.
//!
//!   masonry-probe.exe [--size WxH] [--file note.md] [--metrics file.json] [--trace] [--focus]

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod host;

use std::path::PathBuf;

use masonry::core::{NewWidget, PropertySet, Widget, WidgetTag};
use masonry::layout::Length;
use masonry::parley::style::StyleProperty;
use masonry::properties::types::CrossAxisAlignment;
use masonry::properties::{Background, Padding};
use masonry::peniko::color::AlphaColor;
use masonry::widgets::{Flex, Label, Portal, SizedBox, TextArea};
use winit::dpi::PhysicalSize;

const TREE: &[&str] = &[
    "Inbox",
    "Projects",
    "    Aquilum.md",
    "    Moving off Tauri.md",
    "    Document in Rust.md",
    "Books",
    "Daily notes",
    "    2026-10-07.md",
    "    2026-10-08.md",
    "Templates",
    "README.md",
];

pub const EDITOR_TAG: WidgetTag<TextArea<true>> = WidgetTag::named("editor");

const SAMPLE_TEXT: &str = include_str!("../../../src/sample.md");

pub struct Args {
    pub size: PhysicalSize<u32>,
    pub text: String,
    pub metrics: Option<PathBuf>,
    pub trace: bool,
    /// Focus the text immediately: a measurement with a blinking caret without mouse involvement.
    pub focus: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        size: PhysicalSize::new(2881, 1717),
        text: SAMPLE_TEXT.to_owned(),
        metrics: None,
        trace: false,
        focus: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| panic!("{flag}: missing value"));
        match flag.as_str() {
            "--size" => {
                let v = value();
                let (w, h) = v.split_once('x').expect("--size: expected WxH");
                args.size = PhysicalSize::new(w.parse().expect("width"), h.parse().expect("height"));
            }
            "--file" => args.text = std::fs::read_to_string(value()).expect("--file: cannot read"),
            "--metrics" => args.metrics = Some(value().into()),
            "--trace" => args.trace = true,
            "--focus" => args.focus = true,
            // Measurement-script arguments that apply only to aquilum-native.
            "--threads" | "--bench-frames" => {
                value();
            }
            other => panic!("unknown argument {other}"),
        }
    }
    args
}

pub fn widget_tree(text: &str) -> NewWidget<impl Widget> {
    let sidebar_bg = AlphaColor::from_rgb8(0x18, 0x18, 0x1b);
    let mut tree = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
    for name in TREE {
        tree = tree.with_fixed(Label::new(*name).with_style(StyleProperty::FontSize(14.0)).prepare());
    }
    let sidebar = NewWidget::new(SizedBox::new(NewWidget::new(tree)).width(Length::px(260.0)))
        .with_props(PropertySet::new().with(Background::Color(sidebar_bg)).with(Padding::all(Length::px(12.0))));

    let editor = TextArea::new_editable(text)
        .with_style(StyleProperty::FontSize(16.0))
        .with_style(StyleProperty::LineHeight(masonry::parley::LineHeight::FontSizeRelative(1.6)));
    let column = SizedBox::new(NewWidget::new(editor).with_tag(EDITOR_TAG)).width(Length::px(760.0));
    let content = Portal::new(
        NewWidget::new(Flex::column().with_fixed(NewWidget::new(column)))
            .with_props(PropertySet::new().with(Padding::all(Length::px(32.0)))),
    )
    .prepare();

    NewWidget::new(Flex::row().with_fixed(sidebar).with(content, 1.0))
}

fn main() {
    let args = parse_args();
    host::run(args);
}
