//! Токены дизайн-системы для нативного интерфейса.
//!
//! Источник — файлы токенов в синтаксисе CSS-переменных (`assets/styles/tokens/*.tokens`, темы —
//! `assets/styles/themes/*.tokens`). Расширение не `.css`, потому что это не таблицы стилей: их читает
//! только эта сборка. Здесь объявления `--q-*` вычисляются целиком: `var()`,
//! `color-mix(in srgb, …)`, `calc`/`min`/`max`/`clamp`, `rem` (база 16 px → логические пиксели).
//!
//! Темы: `:root` — светлая тема Aquilum, `[data-theme="dark"]` — тёмная. Другие темы — блоки
//! `[data-theme="id"]` с `--q-theme-name` и `color-scheme`: тёмная (`color-scheme: dark`) строится поверх
//! тёмной темы Aquilum, светлая — поверх `:root`. Селектор может перечислять несколько тем через запятую.
//!
//! Шрифты — `assets/fonts/*.woff2`: распаковываются в TTF, у кириллической
//! половины Inter семейство переименовывается в «Inter Cyrillic» (обе половины называются «Inter»,
//! а fontique из одноимённых шрифтов с одинаковым начертанием берёт один — кириллица ушла бы в
//! системный шрифт; стек «Inter, Inter Cyrillic» подбирает шрифт по кластерам). Список —
//! `$OUT_DIR/fonts.rs`.
//!
//! На выходе `$OUT_DIR/tokens.rs`:
//! - `Theme` — цвета, тени, размеры и насыщенность заголовков (зависят от темы); значения `LIGHT`, `DARK`
//!   и список встроенных тем `THEMES`;
//! - `INPUTS` и `derive` — цвета, которые меняются поверх темы (акцент и правки пользователя), и пересчёт
//!   зависящих от них токенов: зависимость линейна, рецепт снимается пробами (вход прозрачный или белый);
//! - модули `size` (логические пиксели), `number`, `em`, `ms`, `text` — константы, одинаковые
//!   для всех тем;
//! - модуль `motion` — длительности в секундах (`FAST`, `MEDIUM`, `NORMAL`, `MORPH`, `SLOWER`) и кривые
//!   `EASE_*` четвёркой чисел из токенов `--q-duration-*` и `--q-ease-*`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const TOKEN_FILES: &[&str] = &[
    "unit.tokens",
    "colors.tokens",
    "dimension.tokens",
    "typography.tokens",
    "effects.tokens",
    "semantic.tokens",
    "elevations.tokens",
    "components.tokens",
];
const THEME_FILES: &[&str] = &["dark.tokens", "catppuccin.tokens"];
/// Цвета акцента — первые три входа `derive`: основной, `500` и `600` (темнее).
const ACCENT: [&str; 3] = ["blue-alpha-main", "blue-500", "blue-600"];
/// Цвета, которые пользователь меняет поверх темы в настройках.
const EDITABLE: &[&str] = &[
    "bg-canvas",
    "sidebar-bg",
    "tabs-bg",
    "editor-bg",
    "text-primary",
    "text-secondary",
    "text-tertiary",
    "border-default",
    "text-link",
    "editor-code-bg",
    "editor-strong-color",
    "editor-em-color",
    "editor-heading-1-color",
    "editor-heading-2-color",
    "editor-heading-3-color",
    "editor-heading-4-color",
    "editor-heading-5-color",
    "editor-heading-6-color",
];
/// `--q-base-font-size`: 1rem в логических пикселях.
const REM: f64 = 16.0;
/// Значение зависит от окна или родителя (`vh`, `vw`, `ch`, `inherit`): статически не вычисляется,
/// попадает в `text::*` как есть, считается в коде виджета.
const DYNAMIC: &str = "динамическое значение";

/// Числа, которые зависят от темы: размер и насыщенность заголовков.
fn themed_number(name: &str) -> bool {
    name.strip_prefix("editor-heading-").is_some_and(|rest| rest.ends_with("-scale") || rest.ends_with("-weight"))
}

struct Built {
    id: String,
    name: String,
    dark: bool,
    vars: BTreeMap<String, String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Color,
    Shadows,
    Number,
}

struct Field {
    name: String,
    kind: Kind,
    /// По темам, в порядке `THEMES`.
    values: Vec<Value>,
}

type Consts = BTreeMap<&'static str, Vec<(String, String)>>;

fn main() {
    icon();
    fonts();
    let styles = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/styles");
    let mut problems = Vec::new();
    let themes = themes(&styles, &mut problems);
    let (fields, consts) = fields(&themes, &mut problems);
    let inputs: Vec<&str> = ACCENT.iter().chain(EDITABLE).copied().collect();
    for input in &inputs {
        if !fields.iter().any(|f| f.name == *input && f.kind == Kind::Color) {
            problems.push(format!("{input}: не цвет темы, а входит в INPUTS"));
        }
    }
    let derivations: Vec<Derivation> = themes.iter().map(|theme| derivation(theme, &fields, &inputs, &mut problems)).collect();
    // Нерешённые токены не роняют сборку, но видны в выводе cargo.
    for problem in &problems {
        println!("cargo:warning=токен {problem}");
    }
    let out = generate(&themes, &fields, &consts, &inputs, &derivations);
    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("tokens.rs");
    std::fs::write(out_path, out).unwrap();
}

fn theme_id(selector: &str) -> Option<&str> {
    selector.trim().strip_prefix("[data-theme=\"")?.strip_suffix("\"]")
}

/// Встроенные темы: светлая и тёмная Aquilum, затем остальные в порядке объявления.
fn themes(styles: &Path, problems: &mut Vec<String>) -> Vec<Built> {
    let mut files: Vec<PathBuf> = TOKEN_FILES.iter().map(|f| styles.join("tokens").join(f)).collect();
    files.extend(THEME_FILES.iter().map(|f| styles.join("themes").join(f)));
    let mut root = BTreeMap::new();
    let mut overrides: Vec<(String, BTreeMap<String, String>)> = vec![("dark".to_owned(), BTreeMap::new())];
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let css = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        for (selector, name, value) in declarations(&css) {
            if selector == ":root" {
                root.insert(name, value);
                continue;
            }
            for id in selector.split(',').filter_map(theme_id) {
                let at = overrides.iter().position(|(known, _)| known == id).unwrap_or_else(|| {
                    overrides.push((id.to_owned(), BTreeMap::new()));
                    overrides.len() - 1
                });
                overrides[at].1.insert(name.clone(), value.clone());
            }
        }
    }
    let mut themes = vec![Built { id: "light".to_owned(), name: "Aquilum".to_owned(), dark: false, vars: root.clone() }];
    let mut dark_vars = root.clone();
    for (id, mut own) in overrides {
        let dark = own.remove("theme-scheme").is_some_and(|scheme| scheme == "dark");
        let name = own.remove("theme-name").map(|name| name.trim_matches(['\'', '"']).to_owned());
        if id == "dark" {
            dark_vars.extend(own);
            themes.push(Built { id, name: "Aquilum".to_owned(), dark: true, vars: dark_vars.clone() });
            continue;
        }
        let Some(name) = name else {
            problems.push(format!("тема {id}: нет --q-theme-name"));
            continue;
        };
        let mut vars = if dark { dark_vars.clone() } else { root.clone() };
        vars.extend(own);
        themes.push(Built { id, name, dark, vars });
    }
    themes
}

fn same_kind(value: &Value, kind: Kind) -> bool {
    matches!((value, kind), (Value::Color(_), Kind::Color) | (Value::Shadows(_), Kind::Shadows) | (Value::Number(_), Kind::Number))
}

/// Поля `Theme` (зависят от темы) и константы (одинаковы во всех темах).
fn fields(themes: &[Built], problems: &mut Vec<String>) -> (Vec<Field>, Consts) {
    let mut fields = Vec::new();
    let mut consts: Consts = BTreeMap::new();
    for name in themes[0].vars.keys() {
        let values: Vec<Result<Value, String>> = themes.iter().map(|t| Resolver::new(&t.vars).value(name)).collect();
        let kind = match &values[0] {
            Ok(Value::Color(_)) => Some(Kind::Color),
            Ok(Value::Shadows(_)) => Some(Kind::Shadows),
            Ok(Value::Number(_)) if themed_number(name) => Some(Kind::Number),
            _ => None,
        };
        if let (Some(kind), Ok(first)) = (kind, &values[0]) {
            let mut resolved = Vec::new();
            for (theme, value) in themes.iter().zip(&values) {
                match value {
                    Ok(v) if same_kind(v, kind) => resolved.push(v.clone()),
                    Ok(v) => {
                        problems.push(format!("{name}: в теме {} {v:?}", theme.id));
                        resolved.push(first.clone());
                    }
                    Err(e) => {
                        problems.push(format!("{name}: в теме {}: {e}", theme.id));
                        resolved.push(first.clone());
                    }
                }
            }
            fields.push(Field { name: name.clone(), kind, values: resolved });
            continue;
        }
        match &values[0] {
            Ok(l) if values.iter().all(|v| matches!(v, Ok(other) if other == l)) => {
                let (module, ty, literal) = match l {
                    Value::Length(px) => ("size", "f64", float(*px)),
                    Value::Number(n) => ("number", "f64", float(*n)),
                    Value::Em(em) => ("em", "f64", float(*em)),
                    Value::Time(ms) => ("ms", "f64", float(*ms)),
                    Value::Text(s) => ("text", "&str", format!("{s:?}")),
                    Value::Color(_) | Value::Shadows(_) => unreachable!(),
                };
                consts.entry(module).or_default().push((format!("{}: {ty}", ident(name).to_uppercase()), literal));
                let id = ident(name);
                if let (Value::Time(ms), Some(short)) = (l, id.strip_prefix("duration_")) {
                    consts.entry("motion").or_default().push((format!("{}: f64", short.to_uppercase()), float(ms / 1000.0)));
                }
                if let (Value::Text(css), true) = (l, id.starts_with("ease_"))
                    && let Some(curve) = bezier(css)
                {
                    consts.entry("motion").or_default().push((format!("{}: (f64, f64, f64, f64)", id.to_uppercase()), curve));
                }
            }
            Ok(_) if values.iter().all(Result::is_ok) => problems.push(format!("{name}: значение без цвета зависит от темы")),
            Err(e) if e == DYNAMIC => consts
                .entry("text")
                .or_default()
                .push((format!("{}: &str", ident(name).to_uppercase()), format!("{:?}", themes[0].vars[name.as_str()]))),
            _ => {
                let error = values.iter().find_map(|v| v.as_ref().err()).cloned().unwrap_or_default();
                problems.push(format!("{name}: {error}"));
            }
        }
    }
    (fields, consts)
}

fn premultiplied(c: [f64; 4]) -> [f64; 4] {
    [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
}

/// Веса входов: индекс в `INPUTS` и множитель premultiplied-цвета.
type Weights = Vec<(usize, f64)>;

/// Цвет как функция входов: premultiplied-основа и веса входов.
enum Recipe {
    Color([f64; 4], Weights),
    Shadows(Vec<(Shadow, [f64; 4], Weights)>),
}

impl Recipe {
    fn uses(&self) -> Vec<usize> {
        match self {
            Recipe::Color(_, weights) => weights.iter().map(|(k, _)| *k).collect(),
            Recipe::Shadows(list) => list.iter().flat_map(|(_, _, weights)| weights.iter().map(|(k, _)| *k)).collect(),
        }
    }
}

struct Derivation {
    /// Значение входа без правки — в порядке вычисления (вход после тех, от которых зависит).
    inputs: Vec<(usize, Recipe)>,
    /// Остальные поля, зависящие от входов.
    fields: Vec<(String, Recipe)>,
}

/// Все токены, на которые ссылается значение через `var()`, транзитивно.
struct References<'a> {
    vars: &'a BTreeMap<String, String>,
    memo: HashMap<String, BTreeSet<String>>,
}

impl References<'_> {
    fn of(&mut self, name: &str) -> BTreeSet<String> {
        if let Some(known) = self.memo.get(name) {
            return known.clone();
        }
        self.memo.insert(name.to_owned(), BTreeSet::new());
        let raw = self.vars.get(name).cloned().unwrap_or_default();
        let mut found = BTreeSet::new();
        let mut rest = raw.as_str();
        while let Some(at) = rest.find("var(--q-") {
            let tail = &rest[at + "var(--q-".len()..];
            let end = tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).unwrap_or(tail.len());
            found.insert(tail[..end].to_owned());
            found.extend(self.of(&tail[..end]));
            rest = &tail[end..];
        }
        self.memo.insert(name.to_owned(), found.clone());
        found
    }
}

fn linear(base: [f64; 4], white: [f64; 4], name: &str, problems: &mut Vec<String>) -> f64 {
    let (b, w) = (premultiplied(base), premultiplied(white));
    let delta = w[3] - b[3];
    if (0..3).any(|i| (w[i] - b[i] - delta).abs() > 1e-6) {
        problems.push(format!("{name}: цвет зависит от входа нелинейно"));
    }
    delta
}

/// Рецепт `name` по входам `symbolic`: пробы с прозрачными входами и с каждым входом белым.
fn recipe(theme: &Built, name: &str, symbolic: &[usize], inputs: &[&str], problems: &mut Vec<String>) -> Option<Recipe> {
    let resolve = |white: Option<usize>| {
        let overlay: BTreeMap<String, String> =
            symbolic.iter().map(|&k| (inputs[k].to_owned(), if white == Some(k) { "#ffffff" } else { "transparent" }.to_owned())).collect();
        Resolver::layered(&theme.vars, &overlay).value(name)
    };
    let base = resolve(None).map_err(|e| problems.push(format!("{name} в теме {}: {e}", theme.id))).ok()?;
    let whites: Vec<Value> = symbolic.iter().map(|&k| resolve(Some(k))).collect::<Result<_, _>>().ok()?;
    let weights = |base: [f64; 4], white: &dyn Fn(&Value) -> Option<[f64; 4]>, problems: &mut Vec<String>| {
        symbolic
            .iter()
            .zip(&whites)
            .filter_map(|(&k, probe)| Some((k, linear(base, white(probe)?, name, problems))))
            .filter(|(_, weight)| weight.abs() > 1e-9)
            .collect::<Vec<_>>()
    };
    match &base {
        Value::Color(b) => {
            let w = weights(*b, &|probe| if let Value::Color(c) = probe { Some(*c) } else { None }, problems);
            Some(Recipe::Color(premultiplied(*b), w))
        }
        Value::Shadows(list) => {
            let mut shadows = Vec::new();
            for (i, shadow) in list.iter().enumerate() {
                let w = weights(shadow.color, &|probe| if let Value::Shadows(s) = probe { s.get(i).map(|s| s.color) } else { None }, problems);
                shadows.push((shadow.clone(), premultiplied(shadow.color), w));
            }
            Some(Recipe::Shadows(shadows))
        }
        _ => None,
    }
}

fn derivation(theme: &Built, fields: &[Field], inputs: &[&str], problems: &mut Vec<String>) -> Derivation {
    let mut refs = References { vars: &theme.vars, memo: HashMap::new() };
    let mut depends = |name: &str, own: Option<usize>| -> Vec<usize> {
        let found = refs.of(name);
        (0..inputs.len()).filter(|&k| Some(k) != own && found.contains(inputs[k])).collect()
    };
    let mut pending = Vec::new();
    for (j, input) in inputs.iter().enumerate() {
        let symbolic = depends(input, Some(j));
        match recipe(theme, input, &symbolic, inputs, problems) {
            Some(r @ Recipe::Color(..)) => pending.push((j, r)),
            _ => problems.push(format!("{input} в теме {}: вход без рецепта", theme.id)),
        }
    }
    let mut ordered: Vec<(usize, Recipe)> = Vec::new();
    while !pending.is_empty() {
        let ready = pending.iter().position(|(_, r)| r.uses().iter().all(|k| ordered.iter().any(|(placed, _)| placed == k)));
        let Some(at) = ready else {
            problems.push(format!("тема {}: входы зависят друг от друга по кругу", theme.id));
            break;
        };
        ordered.push(pending.remove(at));
    }
    let mut derived = Vec::new();
    for field in fields {
        if field.kind == Kind::Number || inputs.contains(&field.name.as_str()) {
            continue;
        }
        let symbolic = depends(&field.name, None);
        if symbolic.is_empty() {
            continue;
        }
        if let Some(r) = recipe(theme, &field.name, &symbolic, inputs, problems)
            && !r.uses().is_empty()
        {
            derived.push((ident(&field.name), r));
        }
    }
    Derivation { inputs: ordered, fields: derived }
}

fn weights_code(weights: &[(usize, f64)]) -> String {
    let items: Vec<String> = weights.iter().map(|(k, w)| format!("({k}, {w:.6})")).collect();
    format!("&[{}]", items.join(", "))
}

fn recipe_code(out: &mut String, field: &str, recipe: &Recipe) {
    match recipe {
        Recipe::Color(base, weights) => {
            writeln!(out, "    t.{field} = mix({}, {}, &v);", array(base), weights_code(weights)).unwrap();
        }
        Recipe::Shadows(list) => {
            let items: Vec<String> = list
                .iter()
                .map(|(s, base, weights)| {
                    format!(
                        "Shadow {{ x: {}, y: {}, blur: {}, spread: {}, color: mix({}, {}, &v) }}",
                        float(s.x),
                        float(s.y),
                        float(s.blur),
                        float(s.spread),
                        array(base),
                        weights_code(weights)
                    )
                })
                .collect();
            writeln!(out, "    t.{field} = Box::leak(Box::new([{}]));", items.join(", ")).unwrap();
        }
    }
}

fn array<const N: usize>(values: &[f64; N]) -> String {
    let items: Vec<String> = values.iter().map(|v| format!("{:.6}", v)).collect();
    format!("[{}]", items.join(", "))
}

fn theme_const(index: usize, theme: &Built) -> String {
    match index {
        0 => "LIGHT".to_owned(),
        1 => "DARK".to_owned(),
        _ => format!("THEME_{}", ident(&theme.id).to_uppercase()),
    }
}

fn generate(themes: &[Built], fields: &[Field], consts: &Consts, inputs: &[&str], derivations: &[Derivation]) -> String {
    let mut out = String::new();
    out.push_str("// Сгенерировано build.rs из assets/styles — не править.\n\n");
    out.push_str("/// Цвета, тени и заголовки дизайн-системы для одной темы.\n#[derive(Clone, Copy, Debug)]\npub struct Theme {\n");
    for field in fields {
        let ty = match field.kind {
            Kind::Color => "Color",
            Kind::Shadows => "&'static [Shadow]",
            Kind::Number => "f32",
        };
        writeln!(out, "    pub {}: {ty},", ident(&field.name)).unwrap();
    }
    out.push_str("}\n\n");
    for (index, theme) in themes.iter().enumerate() {
        writeln!(out, "pub const {}: Theme = Theme {{", theme_const(index, theme)).unwrap();
        for field in fields {
            writeln!(out, "    {}: {},", ident(&field.name), rust_value(&field.values[index])).unwrap();
        }
        out.push_str("};\n\n");
    }
    writeln!(out, "/// Встроенные темы: светлая и тёмная Aquilum, затем остальные.\npub static THEMES: [Builtin; {}] = [", themes.len()).unwrap();
    for (index, theme) in themes.iter().enumerate() {
        writeln!(out, "    Builtin {{ id: {:?}, name: {:?}, dark: {}, theme: {} }},", theme.id, theme.name, theme.dark, theme_const(index, theme)).unwrap();
    }
    out.push_str("];\n\n");
    let quoted: Vec<String> = inputs.iter().map(|i| format!("{i:?}")).collect();
    writeln!(out, "/// Входы `derive`: три цвета акцента, затем цвета, которые правит пользователь.\npub const INPUTS: [&str; {}] = [{}];\n", inputs.len(), quoted.join(", ")).unwrap();
    writeln!(out, "pub const INPUT_COUNT: usize = {};\n", inputs.len()).unwrap();
    out.push_str("/// Текущее значение входа в теме.\npub fn input_color(t: &Theme, index: usize) -> Color {\n    match index {\n");
    for (k, input) in inputs.iter().enumerate() {
        writeln!(out, "        {k} => t.{},", ident(input)).unwrap();
    }
    out.push_str("        _ => Color::TRANSPARENT,\n    }\n}\n\n");
    let numbers: Vec<&Field> = fields.iter().filter(|f| f.kind == Kind::Number).collect();
    let quoted: Vec<String> = numbers.iter().map(|f| format!("{:?}", f.name)).collect();
    writeln!(out, "/// Числа темы, которые правит пользователь.\npub const NUMBERS: [&str; {}] = [{}];\n", numbers.len(), quoted.join(", ")).unwrap();
    out.push_str("pub fn number_mut(t: &mut Theme, index: usize) -> Option<&mut f32> {\n    match index {\n");
    for (k, field) in numbers.iter().enumerate() {
        writeln!(out, "        {k} => Some(&mut t.{}),", ident(&field.name)).unwrap();
    }
    out.push_str("        _ => None,\n    }\n}\n\n");
    out.push_str("/// Пересчитывает тему `theme` (индекс в `THEMES`) с заданными входами; `None` — значение темы.\n");
    out.push_str("pub fn derive(theme: usize, t: &mut Theme, set: &[Option<[f32; 4]>; INPUT_COUNT]) {\n    match theme {\n");
    for index in 0..themes.len() {
        writeln!(out, "        {index} => derive_{index}(t, set),").unwrap();
    }
    out.push_str("        _ => {}\n    }\n}\n\n");
    for (index, derivation) in derivations.iter().enumerate() {
        writeln!(out, "fn derive_{index}(t: &mut Theme, set: &[Option<[f32; 4]>; INPUT_COUNT]) {{").unwrap();
        out.push_str("    let mut v = [[0.0; 4]; INPUT_COUNT];\n");
        for (k, recipe) in &derivation.inputs {
            if let Recipe::Color(base, weights) = recipe {
                writeln!(out, "    v[{k}] = input(set[{k}], {}, {}, &v);", array(base), weights_code(weights)).unwrap();
            }
        }
        for (k, _) in &derivation.inputs {
            writeln!(out, "    t.{} = color(v[{k}]);", ident(inputs[*k])).unwrap();
        }
        for (field, recipe) in &derivation.fields {
            recipe_code(&mut out, field, recipe);
        }
        out.push_str("}\n\n");
    }
    for (module, items) in consts {
        writeln!(out, "pub mod {module} {{").unwrap();
        for (decl, literal) in items {
            writeln!(out, "    pub const {decl} = {literal};").unwrap();
        }
        out.push_str("}\n\n");
    }
    out
}

/// Шрифты интерфейса и редактора: то же, что подключает `src/fonts/catalog.ts`.
const FONT_FILES: &[&str] = &[
    "inter-latin-wght-normal.woff2",
    "inter-latin-wght-italic.woff2",
    "inter-cyrillic-wght-normal.woff2",
    "inter-cyrillic-wght-italic.woff2",
    "iAWriterMonoV.woff2",
    "iAWriterMonoV-Italic.woff2",
    "iAWriterQuattroV.woff2",
    "iAWriterQuattroV-Italic.woff2",
];

fn fonts() {
    use write_fonts::read::{FontRef, TableProvider};
    use write_fonts::tables::head::{Head, MacStyle};
    use write_fonts::tables::name::Name;
    use write_fonts::tables::os2::{Os2, SelectionFlags};
    use write_fonts::types::NameId;
    use write_fonts::{FontBuilder, OffsetMarker, from_obj::ToOwnedTable};

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("fonts");
    std::fs::create_dir_all(&out).unwrap();
    let mut list = String::from("// Сгенерировано build.rs из assets/fonts — не править.
pub const FONTS: &[&[u8]] = &[
");
    for file in FONT_FILES {
        let path = source.join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        let woff2 = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut ttf = wuff::decompress_woff2(&woff2).unwrap_or_else(|e| panic!("{file}: {e:?}"));
        if file.starts_with("inter-cyrillic") || file.starts_with("iAWriter") {
            let font = FontRef::new(&ttf).unwrap_or_else(|e| panic!("{file}: {e}"));
            let mut name: Name = font.name().unwrap_or_else(|e| panic!("{file}: {e}")).to_owned_table();
            for record in &mut name.name_record {
                if ![NameId::FAMILY_NAME, NameId::TYPOGRAPHIC_FAMILY_NAME].contains(&record.name_id) {
                    continue;
                }
                let renamed = match record.string.strip_prefix("Inter") {
                    Some(rest) => format!("Inter Cyrillic{rest}"),
                    None => record.string.replace(" V", ""),
                };
                record.string = OffsetMarker::new(renamed);
            }
            let mut builder = FontBuilder::new();
            builder.add_table(&name).unwrap_or_else(|e| panic!("{file}: {e}"));
            if file.starts_with("iAWriter") && file.contains("Italic") {
                let mut os2: Os2 = font.os2().unwrap_or_else(|e| panic!("{file}: {e}")).to_owned_table();
                os2.fs_selection = (os2.fs_selection - SelectionFlags::REGULAR) | SelectionFlags::ITALIC;
                let mut head: Head = font.head().unwrap_or_else(|e| panic!("{file}: {e}")).to_owned_table();
                head.mac_style |= MacStyle::ITALIC;
                builder.add_table(&os2).unwrap_or_else(|e| panic!("{file}: {e}"));
                builder.add_table(&head).unwrap_or_else(|e| panic!("{file}: {e}"));
            }
            ttf = builder.copy_missing_tables(font).build();
        }
        let target = out.join(file.replace(".woff2", ".ttf"));
        std::fs::write(&target, &ttf).unwrap();
        writeln!(list, "    include_bytes!({:?}),", target.display().to_string()).unwrap();
    }
    list.push_str("];
");
    std::fs::write(out.join("../fonts.rs"), list).unwrap();
}

/// Объявления `--q-*` с селектором блока, комментарии вырезаны.
fn declarations(css: &str) -> Vec<(String, String, String)> {
    let mut text = String::new();
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        text.push_str(&rest[..start]);
        rest = rest[start..].find("*/").map_or("", |end| &rest[start + end + 2..]);
    }
    text.push_str(rest);

    let mut result = Vec::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find('{') {
        let selector = rest[..open].trim().rsplit('}').next().unwrap_or("").trim().to_owned();
        let Some(close) = rest[open..].find('}') else { break };
        let body = &rest[open + 1..open + close];
        for decl in body.split(';') {
            let decl = decl.trim();
            if let Some(scheme) = decl.strip_prefix("color-scheme:") {
                result.push((selector.clone(), "theme-scheme".to_owned(), scheme.trim().to_owned()));
                continue;
            }
            let Some(name) = decl.strip_prefix("--q-") else { continue };
            let Some((name, value)) = name.split_once(':') else { continue };
            let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
            result.push((selector.clone(), name.trim().to_owned(), value));
        }
        rest = &rest[open + close + 1..];
    }
    result
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    /// RGBA, 0..1, не premultiplied.
    Color([f64; 4]),
    /// Логические пиксели.
    Length(f64),
    Number(f64),
    Em(f64),
    Time(f64),
    Text(String),
    Shadows(Vec<Shadow>),
}

#[derive(Clone, Debug, PartialEq)]
struct Shadow {
    x: f64,
    y: f64,
    blur: f64,
    spread: f64,
    color: [f64; 4],
}

struct Resolver<'a> {
    vars: &'a BTreeMap<String, String>,
    /// Значения поверх `vars` — для проб входов.
    overlay: Option<&'a BTreeMap<String, String>>,
    stack: Vec<String>,
}

impl<'a> Resolver<'a> {
    fn new(vars: &'a BTreeMap<String, String>) -> Self {
        Resolver { vars, overlay: None, stack: Vec::new() }
    }

    fn layered(vars: &'a BTreeMap<String, String>, overlay: &'a BTreeMap<String, String>) -> Self {
        Resolver { vars, overlay: Some(overlay), stack: Vec::new() }
    }

    fn value(&mut self, name: &str) -> Result<Value, String> {
        if self.stack.iter().any(|n| n == name) {
            return Err(format!("цикл через --q-{name}"));
        }
        let raw = self.overlay.and_then(|o| o.get(name)).or_else(|| self.vars.get(name)).ok_or_else(|| format!("нет --q-{name}"))?.clone();
        self.stack.push(name.to_owned());
        let result = self.expression(&raw);
        self.stack.pop();
        result
    }

    /// Значение целиком: список теней через запятую или одно выражение.
    fn expression(&mut self, raw: &str) -> Result<Value, String> {
        let raw = raw.trim();
        if raw.starts_with("cubic-bezier(") || raw.starts_with('\'') || raw.contains("', ") {
            return Ok(Value::Text(raw.to_owned()));
        }
        let parts = split_top(raw, ',');
        let words = split_top(parts[0], ' ');
        let arithmetic = words.iter().any(|w| matches!(*w, "+" | "-" | "*" | "/"));
        if words.len() >= 3 && !arithmetic && !raw.starts_with("color-mix(") {
            let mut shadows = Vec::new();
            for part in parts {
                shadows.extend(self.shadows(part)?);
            }
            return Ok(Value::Shadows(shadows));
        }
        if parts.len() > 1 {
            return Ok(Value::Text(raw.to_owned()));
        }
        let mut parser = Parser { src: raw.as_bytes(), pos: 0 };
        let value = self.sum(&mut parser)?;
        if parser.pos != raw.len() {
            return Err(format!("не разобрано «{}»", &raw[parser.pos..]));
        }
        Ok(value)
    }

    /// Тень: `x y blur [spread] color`; `var()` на месте тени раскрывается в её тени.
    fn shadows(&mut self, raw: &str) -> Result<Vec<Shadow>, String> {
        let words = split_top(raw.trim(), ' ');
        if words.len() == 1 {
            return match self.expression(words[0])? {
                Value::Shadows(s) => Ok(s),
                other => Err(format!("ожидалась тень, получено {other:?}")),
            };
        }
        let mut lengths = Vec::new();
        let mut color = None;
        for word in words {
            match self.expression(word)? {
                Value::Length(px) => lengths.push(px),
                Value::Number(n) if n == 0.0 => lengths.push(0.0),
                Value::Color(c) => color = Some(c),
                other => return Err(format!("часть тени {other:?}")),
            }
        }
        let at = |i: usize| lengths.get(i).copied().unwrap_or(0.0);
        Ok(vec![Shadow { x: at(0), y: at(1), blur: at(2), spread: at(3), color: color.ok_or("тень без цвета")? }])
    }

    fn sum(&mut self, p: &mut Parser) -> Result<Value, String> {
        let mut left = self.product(p)?;
        loop {
            p.skip_ws();
            let op = match p.peek() {
                Some(b'+') => 1.0,
                Some(b'-') if p.src.get(p.pos + 1) == Some(&b' ') => -1.0,
                _ => return Ok(left),
            };
            p.pos += 1;
            let right = self.product(p)?;
            left = arith(left, right, |a, b| a + op * b)?;
        }
    }

    fn product(&mut self, p: &mut Parser) -> Result<Value, String> {
        let mut left = self.atom(p)?;
        loop {
            p.skip_ws();
            match p.peek() {
                Some(b'*') => {
                    p.pos += 1;
                    let right = self.atom(p)?;
                    left = scale(left, right, |a, b| a * b)?;
                }
                Some(b'/') => {
                    p.pos += 1;
                    let right = self.atom(p)?;
                    left = scale(left, right, |a, b| a / b)?;
                }
                _ => return Ok(left),
            }
        }
    }

    fn atom(&mut self, p: &mut Parser) -> Result<Value, String> {
        p.skip_ws();
        if p.peek() == Some(b'(') {
            p.pos += 1;
            let v = self.sum(p)?;
            p.expect(b')')?;
            return Ok(v);
        }
        if p.peek() == Some(b'#') {
            p.pos += 1;
            let hex = p.take_while(|c| c.is_ascii_hexdigit());
            return parse_hex(hex).map(Value::Color);
        }
        if p.peek().is_some_and(|c| c.is_ascii_digit() || c == b'.' || c == b'-') {
            let number = p.take_while(|c| c.is_ascii_digit() || c == b'.' || c == b'-');
            let n: f64 = number.parse().map_err(|_| format!("число «{number}»"))?;
            let unit = p.take_while(|c| c.is_ascii_alphabetic() || c == b'%');
            return match unit {
                "" => Ok(Value::Number(n)),
                "px" => Ok(Value::Length(n)),
                "rem" => Ok(Value::Length(n * REM)),
                "em" => Ok(Value::Em(n)),
                "ms" => Ok(Value::Time(n)),
                "s" => Ok(Value::Time(n * 1000.0)),
                "%" => Ok(Value::Number(n / 100.0)),
                "vh" | "vw" | "ch" => Err(DYNAMIC.into()),
                other => Err(format!("единица «{other}»")),
            };
        }
        let word = p.take_while(|c| c.is_ascii_alphanumeric() || c == b'-');
        if p.peek() != Some(b'(') {
            return match word {
                "transparent" => Ok(Value::Color([0.0; 4])),
                "inherit" => Err(DYNAMIC.into()),
                "white" => Ok(Value::Color([1.0, 1.0, 1.0, 1.0])),
                "black" => Ok(Value::Color([0.0, 0.0, 0.0, 1.0])),
                _ => Err(format!("слово «{word}»")),
            };
        }
        p.pos += 1;
        let args_start = p.pos;
        let mut depth = 1;
        while depth > 0 {
            match p.src.get(p.pos) {
                Some(b'(') => depth += 1,
                Some(b')') => depth -= 1,
                None => return Err("незакрытая скобка".into()),
                _ => {}
            }
            p.pos += 1;
        }
        let args = std::str::from_utf8(&p.src[args_start..p.pos - 1]).unwrap();
        let args = split_top(args, ',');
        match word {
            "var" => {
                let name = args[0].trim().strip_prefix("--q-").ok_or("var() не на --q-")?;
                match self.value(name) {
                    Ok(v) => Ok(v),
                    Err(e) if args.len() > 1 => self.expression(args[1]).map_err(|_| e),
                    Err(e) => Err(e),
                }
            }
            "calc" => self.expression(args[0]),
            "min" | "max" => {
                let mut values = args.iter().map(|a| self.expression(a));
                let mut acc = values.next().ok_or("пустой min/max")??;
                for v in values {
                    let v = v?;
                    acc = arith(acc, v, |a, b| if word == "min" { a.min(b) } else { a.max(b) })?;
                }
                Ok(acc)
            }
            "clamp" => {
                let [lo, mid, hi] = [args[0], args[1], args[2]].map(|a| self.expression(a));
                let v = arith(lo?, mid?, f64::max)?;
                arith(v, hi?, f64::min)
            }
            "rgb" | "rgba" => {
                let parts: Vec<&str> = args.iter().flat_map(|a| a.split_whitespace()).filter(|s| *s != "/").collect();
                let channel = |s: &str| s.trim().parse::<f64>().map_err(|_| format!("канал «{s}»"));
                let alpha = match parts.get(3) {
                    Some(a) if a.ends_with('%') => channel(a.trim_end_matches('%'))? / 100.0,
                    Some(a) => channel(a)?,
                    None => 1.0,
                };
                Ok(Value::Color([channel(parts[0])? / 255.0, channel(parts[1])? / 255.0, channel(parts[2])? / 255.0, alpha]))
            }
            "color-mix" => {
                if args[0].trim() != "in srgb" {
                    return Err(format!("color-mix «{}»", args[0]));
                }
                let (a, pa) = self.mix_part(args[1])?;
                let (b, pb) = self.mix_part(args[2])?;
                let (pa, pb) = match (pa, pb) {
                    (Some(pa), Some(pb)) => (pa, pb),
                    (Some(pa), None) => (pa, 1.0 - pa),
                    (None, Some(pb)) => (1.0 - pb, pb),
                    (None, None) => (0.5, 0.5),
                };
                Ok(Value::Color(mix(a, pa, b, pb)))
            }
            other => Err(format!("функция «{other}»")),
        }
    }

    fn mix_part(&mut self, raw: &str) -> Result<([f64; 4], Option<f64>), String> {
        let words = split_top(raw.trim(), ' ');
        let color = match self.expression(words[0])? {
            Value::Color(c) => c,
            other => return Err(format!("color-mix: {other:?}")),
        };
        let percent = match words.get(1) {
            Some(p) => match p.strip_suffix('%').and_then(|n| n.parse::<f64>().ok()) {
                Some(n) => Some(n / 100.0),
                // `calc(var(--x) * 100%)`: проценты уже переведены в доли.
                None => match self.expression(p)? {
                    Value::Number(n) => Some(n),
                    other => return Err(format!("доля {other:?}")),
                },
            },
            None => None,
        };
        Ok((color, percent))
    }
}

/// Смешение в sRGB по правилам color-mix: в premultiplied-пространстве.
fn mix(a: [f64; 4], pa: f64, b: [f64; 4], pb: f64) -> [f64; 4] {
    let alpha = a[3] * pa + b[3] * pb;
    if alpha == 0.0 {
        return [0.0; 4];
    }
    let channel = |i: usize| (a[i] * a[3] * pa + b[i] * b[3] * pb) / alpha;
    [channel(0), channel(1), channel(2), alpha]
}

fn arith(a: Value, b: Value, f: impl Fn(f64, f64) -> f64) -> Result<Value, String> {
    match (a, b) {
        (Value::Length(a), Value::Length(b)) => Ok(Value::Length(f(a, b))),
        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(f(a, b))),
        (Value::Length(a), Value::Number(b)) if b == 0.0 => Ok(Value::Length(f(a, 0.0))),
        (Value::Number(a), Value::Length(b)) if a == 0.0 => Ok(Value::Length(f(0.0, b))),
        (a, b) => Err(format!("арифметика {a:?} и {b:?}")),
    }
}

fn scale(a: Value, b: Value, f: impl Fn(f64, f64) -> f64) -> Result<Value, String> {
    match (a, b) {
        (Value::Length(a), Value::Number(b)) => Ok(Value::Length(f(a, b))),
        (Value::Number(a), Value::Length(b)) => Ok(Value::Length(f(a, b))),
        (Value::Number(a), Value::Number(b)) => Ok(Value::Number(f(a, b))),
        (a, b) => Err(format!("умножение {a:?} и {b:?}")),
    }
}

fn parse_hex(hex: &str) -> Result<[f64; 4], String> {
    let digit = |i: usize, len: usize| u8::from_str_radix(&hex[i..i + len], 16).map_err(|_| format!("#{hex}"));
    let v = |x: u8| f64::from(x) / 255.0;
    match hex.len() {
        3 | 4 => {
            let c = |i| digit(i, 1).map(|d| v(d * 17));
            Ok([c(0)?, c(1)?, c(2)?, if hex.len() == 4 { c(3)? } else { 1.0 }])
        }
        6 | 8 => {
            let c = |i| digit(i, 2).map(v);
            Ok([c(0)?, c(2)?, c(4)?, if hex.len() == 8 { c(6)? } else { 1.0 }])
        }
        _ => Err(format!("#{hex}")),
    }
}

/// Делит по разделителю вне скобок и кавычек.
fn split_top(s: &str, sep: char) -> Vec<&str> {
    let (mut parts, mut depth, mut start, mut quote) = (Vec::new(), 0, 0, false);
    for (i, c) in s.char_indices() {
        match c {
            '\'' | '"' => quote = !quote,
            '(' if !quote => depth += 1,
            ')' if !quote => depth -= 1,
            c if c == sep && depth == 0 && !quote => {
                if !s[start..i].trim().is_empty() {
                    parts.push(s[start..i].trim());
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    if !s[start..].trim().is_empty() {
        parts.push(s[start..].trim());
    }
    if parts.is_empty() {
        parts.push("");
    }
    parts
}

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn take_while(&mut self, f: impl Fn(u8) -> bool) -> &'a str {
        let start = self.pos;
        while self.peek().is_some_and(&f) {
            self.pos += 1;
        }
        std::str::from_utf8(&self.src[start..self.pos]).unwrap()
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("ожидался «{}»", c as char))
        }
    }
}

fn ident(name: &str) -> String {
    let id = name.replace('-', "_");
    if id.starts_with(|c: char| c.is_ascii_digit()) { format!("_{id}") } else { id }
}

fn bezier(css: &str) -> Option<String> {
    let inner = css.strip_prefix("cubic-bezier(")?.strip_suffix(')')?;
    let numbers: Vec<f64> = inner.split(',').map(|n| n.trim().parse().ok()).collect::<Option<_>>()?;
    let [a, b, c, d] = numbers[..] else { return None };
    Some(format!("({}, {}, {}, {})", float(a), float(b), float(c), float(d)))
}

fn float(v: f64) -> String {
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") { s } else { format!("{s}.0") }
}

fn rgba8(c: [f64; 4]) -> String {
    let b = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("Color::from_rgba8({}, {}, {}, {})", b(c[0]), b(c[1]), b(c[2]), b(c[3]))
}

fn rust_value(v: &Value) -> String {
    match v {
        Value::Color(c) => rgba8(*c),
        Value::Shadows(list) => {
            let items: Vec<String> = list
                .iter()
                .map(|s| {
                    format!(
                        "Shadow {{ x: {}, y: {}, blur: {}, spread: {}, color: {} }}",
                        float(s.x),
                        float(s.y),
                        float(s.blur),
                        float(s.spread),
                        rgba8(s.color)
                    )
                })
                .collect();
            format!("&[{}]", items.join(", "))
        }
        Value::Number(n) => format!("{:?}", *n as f32),
        // Значение темы обязано быть цветом, тенями или числом; несовпадения отсеяны выше.
        other => panic!("значение темы {other:?}"),
    }
}

fn icon() {
    let target_windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    if !target_windows || !msvc {
        return;
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icon.ico");
    println!("cargo:rerun-if-changed={}", source.display());
    let ico = std::fs::read(&source).expect("assets/icon.ico");
    let u16_at = |at: usize| u16::from_le_bytes([ico[at], ico[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes([ico[at], ico[at + 1], ico[at + 2], ico[at + 3]]);
    let count = u16_at(4) as usize;
    let mut res = Vec::new();
    entry(&mut res, 0, 0, 0, &[]);
    let mut group = vec![0, 0, 1, 0];
    group.extend_from_slice(&(count as u16).to_le_bytes());
    for i in 0..count {
        let at = 6 + 16 * i;
        let size = u32_at(at + 8) as usize;
        let offset = u32_at(at + 12) as usize;
        entry(&mut res, 3, i as u16 + 1, 0x1010, &ico[offset..offset + size]);
        group.extend_from_slice(&ico[at..at + 12]);
        group.extend_from_slice(&(i as u16 + 1).to_le_bytes());
    }
    entry(&mut res, 14, 1, 0x1030, &group);
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("icon.res");
    std::fs::write(&out, res).expect("icon.res");
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

fn entry(res: &mut Vec<u8>, kind: u16, name: u16, flags: u16, data: &[u8]) {
    res.extend_from_slice(&(data.len() as u32).to_le_bytes());
    res.extend_from_slice(&32u32.to_le_bytes());
    for id in [kind, name] {
        res.extend_from_slice(&0xFFFFu16.to_le_bytes());
        res.extend_from_slice(&id.to_le_bytes());
    }
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(&flags.to_le_bytes());
    res.extend_from_slice(&(if kind == 0 { 0u16 } else { 0x0409 }).to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(data);
    while res.len() % 4 != 0 {
        res.push(0);
    }
}
