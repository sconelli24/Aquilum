use crate::search::pages::{read_chars, Pages};
use serde_json::{json, Value};
use std::ops::Range;

pub fn text(arguments: &Value, key: &str) -> Result<String, String> {
    optional_text(arguments, key)
        .or_else(|| alias(key).and_then(|other| optional_text(arguments, other)))
        .ok_or_else(|| missing(arguments, key))
}

fn alias(key: &str) -> Option<&'static str> {
    match key {
        "note" => Some("path"),
        "path" => Some("note"),
        _ => None,
    }
}

pub fn optional_text(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

pub fn content(arguments: &Value, key: &str) -> Result<String, String> {
    optional_content(arguments, key).ok_or_else(|| missing(arguments, key))
}

pub fn content_or_empty(arguments: &Value, key: &str) -> Result<String, String> {
    arguments.get(key).and_then(Value::as_str).map(str::to_owned).ok_or_else(|| missing(arguments, key))
}

pub fn optional_content(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn missing(arguments: &Value, key: &str) -> String {
    let given = arguments
        .as_object()
        .map(|fields| fields.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    if given.is_empty() {
        return format!("Argument “{key}” is missing: no arguments were passed at all");
    }
    format!(
        "Argument “{key}” is missing; received: {}",
        given.join(", ")
    )
}

pub fn optional_names(
    arguments: &Value,
    key: &str,
) -> Result<Option<Vec<String>>, String> {
    let Some(value) = arguments.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let list = value
        .as_array()
        .ok_or_else(|| format!("Argument “{key}” must be a list of strings"))?;
    if list.is_empty() {
        return Err(format!("List “{key}” is empty"));
    }
    list.iter()
        .map(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| format!("Argument “{key}”: list strings must not be empty"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

pub fn flag(arguments: &Value, key: &str, fallback: bool) -> bool {
    arguments.get(key).and_then(Value::as_bool).unwrap_or(fallback)
}

pub fn offset(arguments: &Value) -> usize {
    arguments
        .get("offset")
        .and_then(Value::as_u64)
        .map_or(0, |value| usize::try_from(value).unwrap_or(usize::MAX))
}

pub fn limit(arguments: &Value, fallback: usize, maximum: usize) -> usize {
    arguments
        .get("limit")
        .and_then(Value::as_u64)
        .map_or(fallback, |value| value as usize)
        .clamp(1, maximum)
}

pub fn listing<T: serde::Serialize>(key: &str, total: usize, offset: usize, page: Vec<T>) -> Value {
    let end = offset.saturating_add(page.len());
    json!({ "total": total, key: page, "nextOffset": (end < total).then_some(end) })
}

pub fn text_window(pages: &mut Pages<'_>, section: Range<usize>, arguments: &Value, size: usize) -> Result<Value, String> {
    let offset = offset(arguments);
    let from = section.start.saturating_add(offset);
    let (body, chars) = read_chars(pages, from..from.saturating_add(size).min(section.end)).map_err(|error| error.to_string())?;
    let total = section.end.min(chars).saturating_sub(section.start);
    if offset > total {
        return Err(format!("Offset {offset} is past the end of the text: {total} characters in total"));
    }
    let end = offset + size.min(total - offset);
    Ok(json!({ "content": body, "totalChars": total, "truncated": end < total, "nextOffset": (end < total).then_some(end) }))
}
