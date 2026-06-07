use crate::lineage::LineageNode;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, PartialEq)]
pub enum CursorTarget {
    Ref(String),                    // ref('model_name')
    Source(String, String),         // source('source_name', 'table_name')
}

static REF_RE: OnceLock<Regex> = OnceLock::new();
static SOURCE_RE: OnceLock<Regex> = OnceLock::new();

fn ref_re() -> &'static Regex {
    REF_RE.get_or_init(|| Regex::new(r#"ref\(\s*['"](\w+)['"]\s*\)"#).unwrap())
}

fn source_re() -> &'static Regex {
    SOURCE_RE.get_or_init(|| {
        Regex::new(r#"source\(\s*['"](\w+)['"]\s*,\s*['"](\w+)['"]\s*\)"#).unwrap()
    })
}

/// Given a line of SQL and the character offset of the cursor, return the
/// ref/source call that contains the cursor, if any.
pub fn parse_cursor(line: &str, char_offset: usize) -> Option<CursorTarget> {
    for cap in ref_re().captures_iter(line) {
        let m = cap.get(0).unwrap();
        if m.start() <= char_offset && char_offset <= m.end() {
            return Some(CursorTarget::Ref(cap[1].to_string()));
        }
    }
    for cap in source_re().captures_iter(line) {
        let m = cap.get(0).unwrap();
        if m.start() <= char_offset && char_offset <= m.end() {
            return Some(CursorTarget::Source(cap[1].to_string(), cap[2].to_string()));
        }
    }
    None
}

/// Render the lineage tree as Markdown for the hover popup.
pub fn render_hover(
    name: &str,
    file_path: &str,
    upstream: &[LineageNode],
    downstream: &[LineageNode],
) -> String {
    let mut out = format!("## {}\n📁 {}\n", name, file_path);

    if !upstream.is_empty() {
        out.push_str("\n**Upstream**\n");
        render_tree(&mut out, upstream, 0);
    }

    if !downstream.is_empty() {
        out.push_str("\n**Downstream**\n");
        render_tree(&mut out, downstream, 0);
    }

    if upstream.is_empty() && downstream.is_empty() {
        out.push_str("\n_No lineage found._\n");
    }

    out
}

fn render_tree(out: &mut String, nodes: &[LineageNode], indent: usize) {
    let prefix = "    ".repeat(indent);
    for (i, node) in nodes.iter().enumerate() {
        let connector = if i + 1 < nodes.len() { "├── " } else { "└── " };
        let label = match node.source_name.as_deref() {
            Some(src) => format!("{}  [source: {}]", node.name, src),
            None => node.name.clone(),
        };
        out.push_str(&format!("{}{}{}\n", prefix, connector, label));
        render_tree(out, &node.children, indent + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ref() {
        let line = "from {{ ref('orders') }}";
        // cursor inside "orders"
        let target = parse_cursor(line, 12).unwrap();
        assert_eq!(target, CursorTarget::Ref("orders".to_string()));
    }

    #[test]
    fn test_parse_source() {
        let line = r#"from {{ source('jaffle_shop', 'raw_orders') }}"#;
        let target = parse_cursor(line, 20).unwrap();
        assert_eq!(
            target,
            CursorTarget::Source("jaffle_shop".to_string(), "raw_orders".to_string())
        );
    }

    #[test]
    fn test_parse_no_match() {
        let line = "select * from my_table";
        assert!(parse_cursor(line, 10).is_none());
    }

    #[test]
    fn test_cursor_outside_ref_returns_none() {
        let line = "select 1, {{ ref('orders') }}";
        // cursor at position 0 (before the ref)
        assert!(parse_cursor(line, 0).is_none());
    }

    #[test]
    fn test_render_hover_with_lineage() {
        let upstream = vec![LineageNode {
            unique_id: "model.proj.stg_orders".to_string(),
            name: "stg_orders".to_string(),
            resource_type: "model".to_string(),
            source_name: None,
            file_path: "models/stg_orders.sql".to_string(),
            depth: 1,
            children: vec![],
        }];
        let downstream: Vec<LineageNode> = vec![];
        let md = render_hover("orders", "models/orders.sql", &upstream, &downstream);
        assert!(md.contains("## orders"));
        assert!(md.contains("📁 models/orders.sql"));
        assert!(md.contains("**Upstream**"));
        assert!(md.contains("stg_orders"));
    }

    #[test]
    fn test_render_hover_no_lineage() {
        let md = render_hover("isolated", "models/isolated.sql", &[], &[]);
        assert!(md.contains("_No lineage found._"));
    }
}
