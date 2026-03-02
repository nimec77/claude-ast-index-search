//! Dart error recovery for tree-sitter parse errors
use tree_sitter::Node;

use super::super::{line_text, node_line, node_text};
use super::walk_body_declarations;
use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;

/// Try to recover declarations from ERROR nodes.
/// tree-sitter-dart 0.0.4 doesn't understand Dart 3 modifiers:
/// - sealed class, base class, final class → ERROR + block sibling
/// - extension type → ERROR + block sibling
pub(super) fn try_recover_from_error(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let text = node_text(content, node).trim().to_string();
    let line = node_line(node);

    // Check for "sealed class X", "base class X", "final class X"
    if let Some(class_info) = try_parse_modified_class(&text) {
        let sig_line = line_text(content, line).trim().to_string();
        // Try to find block sibling for body methods
        let mut parents = Vec::new();
        // Parse parents from the text after the class name
        parse_parents_from_class_text(&text, &mut parents);

        symbols.push(ParsedSymbol {
            name: class_info.name,
            kind: class_info.kind,
            line,
            signature: sig_line,
            parents,
        });

        // Walk the next sibling (block node) for body declarations
        if let Some(next) = node.next_sibling()
            && next.kind() == "block"
        {
            walk_body_declarations(&next, content, symbols);
        }
    }

    // Check for "extension type X(...) implements Y"
    if (text.starts_with("extension type ") || text.starts_with("extension  type "))
        && let Some(ext_type_info) = try_parse_extension_type(&text)
    {
        let sig_line = line_text(content, line).trim().to_string();
        symbols.push(ParsedSymbol {
            name: ext_type_info.name,
            kind: SymbolKind::Class,
            line,
            signature: sig_line,
            parents: ext_type_info.parents,
        });
    }
}

struct ClassInfo {
    name: String,
    kind: SymbolKind,
}

struct ExtTypeInfo {
    name: String,
    parents: Vec<(String, String)>,
}

/// Try to parse "sealed/base/final class ClassName" from ERROR text
fn try_parse_modified_class(text: &str) -> Option<ClassInfo> {
    // Patterns: "sealed class X", "base class X", "final class X",
    //           "abstract sealed class X", etc.
    let words: Vec<&str> = text.split_whitespace().collect();

    // Find "class" keyword
    let class_idx = words.iter().position(|w| *w == "class")?;
    if class_idx + 1 >= words.len() {
        return None;
    }

    let name = words[class_idx + 1].to_string();
    // Strip generic parameters
    let name = name.split('<').next().unwrap_or(&name).trim().to_string();

    if name.is_empty() {
        return None;
    }

    // Check for modifiers before "class"
    let modifiers: Vec<&str> = words[..class_idx].to_vec();
    let kind = if modifiers.contains(&"interface") {
        SymbolKind::Interface
    } else {
        SymbolKind::Class
    };

    Some(ClassInfo { name, kind })
}

/// Parse parents from class declaration text (after class name)
fn parse_parents_from_class_text(text: &str, parents: &mut Vec<(String, String)>) {
    // Find "extends", "with", "implements" in the text
    let parts = text.split_whitespace().collect::<Vec<_>>();

    let mut mode = "";
    for &word in &parts {
        match word {
            "extends" => {
                mode = "extends";
                continue;
            }
            "with" => {
                mode = "with";
                continue;
            }
            "implements" => {
                mode = "implements";
                continue;
            }
            "class" | "sealed" | "base" | "final" | "abstract" | "interface" => continue,
            _ => {}
        }
        if !mode.is_empty() {
            // This word is a type name
            let name = word
                .trim_end_matches(',')
                .split('<')
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() && name != "{" && name != "}" {
                parents.push((name.to_string(), mode.to_string()));
            }
        }
    }
}

/// Try to parse "extension type X(...) implements Y" from ERROR text
fn try_parse_extension_type(text: &str) -> Option<ExtTypeInfo> {
    let words: Vec<&str> = text.split_whitespace().collect();

    // Find "type" keyword after "extension"
    let type_idx = words.iter().position(|w| *w == "type")?;
    if type_idx + 1 >= words.len() {
        return None;
    }

    let name_raw = words[type_idx + 1];
    let name = name_raw
        .split('(')
        .next()
        .unwrap_or(name_raw)
        .trim()
        .to_string();

    if name.is_empty() {
        return None;
    }

    let mut parents = Vec::new();
    if let Some(impl_idx) = words.iter().position(|w| *w == "implements") {
        for &word in &words[impl_idx + 1..] {
            let type_name = word
                .trim_end_matches(',')
                .split('<')
                .next()
                .unwrap_or("")
                .trim();
            if !type_name.is_empty() && type_name != "{" && type_name != "}" {
                parents.push((type_name.to_string(), "implements".to_string()));
            }
        }
    }

    Some(ExtTypeInfo { name, parents })
}

/// Extract parents from ERROR node text (for enum with/implements in tree-sitter-dart 0.0.4)
pub(super) fn extract_parents_from_error_text(
    node: &Node,
    content: &str,
    parents: &mut Vec<(String, String)>,
) {
    let text = node_text(content, node);
    let words: Vec<&str> = text.split_whitespace().collect();

    let mut mode = "";
    for &word in &words {
        match word {
            "with" => {
                mode = "with";
                continue;
            }
            "implements" => {
                mode = "implements";
                continue;
            }
            _ => {}
        }
        if !mode.is_empty() {
            let name = word
                .trim_end_matches(',')
                .split('<')
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                parents.push((name.to_string(), mode.to_string()));
            }
        }
    }
}

/// Find first identifier child node and return its text
pub(super) fn find_first_identifier(node: &Node, content: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return Some(node_text(content, &child).to_string());
        }
    }
    None
}

/// Find first type_identifier in descendants
pub(super) fn find_first_type_identifier(node: &Node, content: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" {
            return Some(node_text(content, &child).to_string());
        }
        if let Some(found) = find_first_type_identifier(&child, content) {
            return Some(found);
        }
    }
    None
}

/// Find a descendant node by kind
pub(super) fn find_descendant_by_kind<'a>(node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            return Some(child);
        }
        if let Some(found) = find_descendant_by_kind(&child, kind) {
            return Some(found);
        }
    }
    None
}
