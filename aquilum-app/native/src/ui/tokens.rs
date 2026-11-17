//! Токены дизайн-системы, сгенерированные `build.rs` из CSS-токенов фронтенда
//! (`assets/styles/tokens/*.tokens`, темы — `assets/styles/themes/*.tokens`). Размеры — в логических пикселях
//! при масштабе интерфейса 1 (1rem = 16 px); масштаб применяется ко всему окну сразу.
//!
//! Это вся дизайн-система целиком: интерфейс использует её по мере переноса.
#![allow(dead_code)]

use masonry::peniko::Color;

/// Тень из токенов (`box-shadow`): смещение, размытие, растяжение — в логических пикселях.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub x: f64,
    pub y: f64,
    pub blur: f64,
    pub spread: f64,
    pub color: Color,
}

/// Встроенная тема: идентификатор для настроек, название, светлая или тёмная, значения токенов.
pub struct Builtin {
    pub id: &'static str,
    pub name: &'static str,
    pub dark: bool,
    pub theme: Theme,
}

fn premultiply(c: [f32; 4]) -> [f32; 4] {
    [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
}

/// Premultiplied-основа плюс входы `v` (premultiplied) с весами.
fn combine(base: [f32; 4], weights: &[(usize, f32)], v: &[[f32; 4]]) -> [f32; 4] {
    let mut out = base;
    for &(k, weight) in weights {
        for (channel, value) in out.iter_mut().zip(v[k]) {
            *channel += weight * value;
        }
    }
    out
}

/// Вход `derive`: правка пользователя (не premultiplied) или значение темы по рецепту.
fn input(set: Option<[f32; 4]>, base: [f32; 4], weights: &[(usize, f32)], v: &[[f32; 4]]) -> [f32; 4] {
    set.map_or_else(|| combine(base, weights, v), premultiply)
}

fn color(premultiplied: [f32; 4]) -> Color {
    let alpha = premultiplied[3].clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return Color::TRANSPARENT;
    }
    let channel = |i: usize| (premultiplied[i] / alpha).clamp(0.0, 1.0);
    Color::new([channel(0), channel(1), channel(2), alpha])
}

fn mix(base: [f32; 4], weights: &[(usize, f32)], v: &[[f32; 4]]) -> Color {
    color(combine(base, weights, v))
}

/// Светлая (`dark == false`) или тёмная тема Aquilum с другим акцентом: основной цвет, `500`, `600`.
pub fn recolor(theme: &mut Theme, dark: bool, accent: [[f32; 4]; 3]) {
    let mut set = [None; INPUT_COUNT];
    for (slot, color) in set.iter_mut().zip(accent) {
        *slot = Some(color);
    }
    derive(usize::from(dark), theme, &set);
}

include!(concat!(env!("OUT_DIR"), "/tokens.rs"));
