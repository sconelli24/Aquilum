use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Canvas {
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Text,
    File,
    Link,
    Group,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    /// Kept as written so unknown node types survive a round trip.
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subpath: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default, rename = "backgroundStyle", skip_serializing_if = "Option::is_none")]
    pub background_style: Option<String>,
    #[serde(serialize_with = "number", deserialize_with = "lenient")]
    pub x: f64,
    #[serde(serialize_with = "number", deserialize_with = "lenient")]
    pub y: f64,
    #[serde(serialize_with = "number", deserialize_with = "lenient")]
    pub width: f64,
    #[serde(serialize_with = "number", deserialize_with = "lenient")]
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum End {
    None,
    Arrow,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    #[serde(rename = "fromNode")]
    pub from_node: String,
    #[serde(default, rename = "fromSide", skip_serializing_if = "Option::is_none")]
    pub from_side: Option<Side>,
    #[serde(default, rename = "fromEnd", skip_serializing_if = "Option::is_none")]
    pub from_end: Option<End>,
    #[serde(rename = "toNode")]
    pub to_node: String,
    #[serde(default, rename = "toSide", skip_serializing_if = "Option::is_none")]
    pub to_side: Option<Side>,
    #[serde(default, rename = "toEnd", skip_serializing_if = "Option::is_none")]
    pub to_end: Option<End>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Whole numbers are written without a fraction, as Obsidian writes them.
fn number<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    if value.fract() == 0.0 && value.abs() < 9.0e15 {
        serializer.serialize_i64(*value as i64)
    } else {
        serializer.serialize_f64(*value)
    }
}

/// Accepts numbers and numeric strings; anything else becomes 0 instead of failing the file.
fn lenient<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(n) => n.as_f64().unwrap_or_default(),
        Value::String(s) => s.trim().parse().unwrap_or_default(),
        _ => 0.0,
    })
}

impl Canvas {
    pub fn parse(text: &str) -> Result<Canvas, serde_json::Error> {
        if text.trim().is_empty() {
            return Ok(Canvas::default());
        }
        serde_json::from_str(text)
    }

    /// Tab-indented JSON, the layout Obsidian writes.
    pub fn to_json(&self) -> String {
        let mut out = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
        let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
        // Serializing plain data into a Vec cannot fail.
        let _ = self.serialize(&mut serializer);
        String::from_utf8(out).unwrap_or_default()
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Removes the nodes and every edge touching them.
    pub fn remove_nodes(&mut self, ids: &[String]) {
        self.nodes.retain(|n| !ids.contains(&n.id));
        self.edges.retain(|e| !ids.contains(&e.from_node) && !ids.contains(&e.to_node));
    }

    /// Bounding box of all nodes as (min_x, min_y, max_x, max_y).
    pub fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        self.nodes.iter().fold(None, |acc, n| {
            let (x0, y0, x1, y1) = (n.x, n.y, n.x + n.width, n.y + n.height);
            Some(match acc {
                None => (x0, y0, x1, y1),
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            })
        })
    }

    /// Vault-relative paths of file nodes, in node order.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().filter(|n| n.kind() == NodeKind::File).filter_map(|n| n.file.as_deref())
    }
}

impl Node {
    pub fn kind(&self) -> NodeKind {
        match self.kind.as_str() {
            "text" => NodeKind::Text,
            "file" => NodeKind::File,
            "link" => NodeKind::Link,
            "group" => NodeKind::Group,
            _ => NodeKind::Other,
        }
    }

    fn base(kind: &str, x: f64, y: f64, width: f64, height: f64) -> Node {
        Node {
            id: super::new_id(),
            kind: kind.to_owned(),
            text: None,
            file: None,
            subpath: None,
            url: None,
            label: None,
            background: None,
            background_style: None,
            x,
            y,
            width,
            height,
            color: None,
            extra: Map::new(),
        }
    }

    pub fn text(text: &str, x: f64, y: f64, width: f64, height: f64) -> Node {
        Node { text: Some(text.to_owned()), ..Node::base("text", x, y, width, height) }
    }

    pub fn file(file: &str, x: f64, y: f64, width: f64, height: f64) -> Node {
        Node { file: Some(file.to_owned()), ..Node::base("file", x, y, width, height) }
    }

    pub fn link(url: &str, x: f64, y: f64, width: f64, height: f64) -> Node {
        Node { url: Some(url.to_owned()), ..Node::base("link", x, y, width, height) }
    }

    pub fn group(label: Option<&str>, x: f64, y: f64, width: f64, height: f64) -> Node {
        Node { label: label.map(str::to_owned), ..Node::base("group", x, y, width, height) }
    }

    /// Whether `other` lies entirely inside this node; used for group membership.
    pub fn contains(&self, other: &Node) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.x + other.width <= self.x + self.width
            && other.y + other.height <= self.y + self.height
    }
}

impl Edge {
    pub fn new(from_node: &str, from_side: Option<Side>, to_node: &str, to_side: Option<Side>) -> Edge {
        Edge {
            id: super::new_id(),
            from_node: from_node.to_owned(),
            from_side,
            from_end: None,
            to_node: to_node.to_owned(),
            to_side,
            to_end: None,
            color: None,
            label: None,
            extra: Map::new(),
        }
    }

    /// The spec defaults: no marker at the start, an arrow at the end.
    pub fn from_arrow(&self) -> bool {
        self.from_end == Some(End::Arrow)
    }

    pub fn to_arrow(&self) -> bool {
        self.to_end != Some(End::None)
    }
}
