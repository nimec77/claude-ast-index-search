//! Tree-sitter based C# parser

use anyhow::Result;
use std::sync::LazyLock;
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator};

use super::{LanguageParser, find_capture, line_text, node_line, node_text, parse_tree};
use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;

static CSHARP_LANGUAGE: LazyLock<Language> = LazyLock::new(|| tree_sitter_c_sharp::LANGUAGE.into());

static CSHARP_QUERY: LazyLock<Query> = LazyLock::new(|| {
    Query::new(&CSHARP_LANGUAGE, include_str!("queries/csharp.scm"))
        .expect("Failed to compile C# tree-sitter query")
});

pub static CSHARP_PARSER: CSharpParser = CSharpParser;

pub struct CSharpParser;

/// Significant C# attributes that are worth tracking
fn is_significant_attr(name: &str) -> bool {
    matches!(
        name,
        "Serializable"
            | "DataContract"
            | "DataMember"
            | "JsonProperty"
            | "JsonIgnore"
            | "Required"
            | "Authorize"
            | "AllowAnonymous"
            | "HttpGet"
            | "HttpPost"
            | "HttpPut"
            | "HttpDelete"
            | "Route"
            | "ApiController"
            | "Controller"
            | "Test"
            | "TestMethod"
            | "Fact"
            | "Theory"
            | "SerializeField"
            | "Header"
            | "Tooltip"
            | "Range"
            | "DllImport"
            | "StructLayout"
            | "MarshalAs"
            | "Obsolete"
            | "Conditional"
            | "DebuggerDisplay"
    )
}

/// Check if a C# name looks like an interface (starts with I + uppercase)
fn is_interface_name(name: &str) -> bool {
    name.starts_with('I')
        && name.len() > 1
        && name
            .chars()
            .nth(1)
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
}

/// Parse base_list node to extract parent type names with their relationship kind.
/// In C#, the base_list contains types separated by commas.
/// Convention: names starting with I+uppercase are "implements", others are "extends".
fn parse_base_list(content: &str, node: &tree_sitter::Node) -> Vec<(String, String)> {
    let mut parents = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();

        // Skip punctuation and argument_list
        if kind == "," || kind == ":" || kind == "argument_list" {
            continue;
        }

        // For primary_constructor_base_type (e.g. `Person(Name)` in record bases),
        // extract the type from the first named child
        if kind == "primary_constructor_base_type" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                let inner_kind = inner_child.kind();
                if inner_kind != "argument_list"
                    && inner_kind != ","
                    && inner_kind != "("
                    && inner_kind != ")"
                {
                    let type_name = extract_type_name(content, &inner_child);
                    if !type_name.is_empty() {
                        let rel = if is_interface_name(&type_name) {
                            "implements".to_string()
                        } else {
                            "extends".to_string()
                        };
                        parents.push((type_name, rel));
                        break;
                    }
                }
            }
            continue;
        }

        // Extract the type name
        let type_name = extract_type_name(content, &child);
        if !type_name.is_empty() {
            let rel = if is_interface_name(&type_name) {
                "implements".to_string()
            } else {
                "extends".to_string()
            };
            parents.push((type_name, rel));
        }
    }
    parents
}

/// Extract a clean type name from a type node, stripping generic parameters.
/// e.g. "IRepository<T>" -> "IRepository", "BaseEntity" -> "BaseEntity"
fn extract_type_name(content: &str, node: &tree_sitter::Node) -> String {
    match node.kind() {
        "identifier" => node_text(content, node).to_string(),
        "qualified_name" => node_text(content, node).to_string(),
        "generic_name" => {
            // For generic_name, just take the identifier part (first child)
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    return node_text(content, &child).to_string();
                }
            }
            node_text(content, node).to_string()
        }
        "predefined_type" => node_text(content, node).to_string(),
        _ => {
            // For other node types, try to get the text directly
            let text = node_text(content, node).trim().to_string();
            // Strip generic parameters if present
            if let Some(idx) = text.find('<') {
                text[..idx].to_string()
            } else {
                text
            }
        }
    }
}

/// Extract variable names from a field_declaration or event_field_declaration node.
/// These nodes contain: modifiers, variable_declaration { type, variable_declarator { name } }
fn extract_field_info(content: &str, node: &tree_sitter::Node) -> Vec<(String, usize, bool)> {
    let mut results = Vec::new();
    let mut has_const = false;

    // Check modifiers for const
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifier" {
            let mod_text = node_text(content, &child);
            if mod_text == "const" {
                has_const = true;
            }
        }
        if child.kind() == "variable_declaration" {
            let mut inner_cursor = child.walk();
            for var_child in child.children(&mut inner_cursor) {
                if var_child.kind() == "variable_declarator" {
                    // Get the name field
                    if let Some(name_node) = var_child.child_by_field_name("name") {
                        let name = node_text(content, &name_node).to_string();
                        let line = node_line(&name_node);
                        results.push((name, line, has_const));
                    }
                }
            }
        }
    }
    results
}

/// Extract event field variable names from an event_field_declaration node.
fn extract_event_field_names(content: &str, node: &tree_sitter::Node) -> Vec<(String, usize)> {
    let mut results = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declaration" {
            let mut inner_cursor = child.walk();
            for var_child in child.children(&mut inner_cursor) {
                if var_child.kind() == "variable_declarator"
                    && let Some(name_node) = var_child.child_by_field_name("name")
                {
                    let name = node_text(content, &name_node).to_string();
                    let line = node_line(&name_node);
                    results.push((name, line));
                }
            }
        }
    }
    results
}

/// Extract the name from a using_directive node.
/// Handles both `using Foo.Bar;` and `using Alias = Foo.Bar;`
fn extract_using_name(content: &str, node: &tree_sitter::Node) -> Option<(String, usize)> {
    let mut cursor = node.walk();
    let line = node_line(node);

    // Walk children to find the name/qualified_name
    for child in node.children(&mut cursor) {
        match child.kind() {
            "qualified_name" | "identifier" => {
                let name = node_text(content, &child).to_string();
                return Some((name, line));
            }
            _ => {}
        }
    }
    None
}

impl LanguageParser for CSharpParser {
    fn parse_symbols(&self, content: &str) -> Result<Vec<ParsedSymbol>> {
        let tree = parse_tree(content, &CSHARP_LANGUAGE)?;
        let mut symbols = Vec::new();
        let mut cursor = QueryCursor::new();
        let query = &*CSHARP_QUERY;

        // Build capture name -> index map
        let capture_names = query.capture_names();
        let idx = |name: &str| -> Option<u32> {
            capture_names
                .iter()
                .position(|n| *n == name)
                .map(|i| i as u32)
        };

        let idx_namespace_name = idx("namespace_name");
        let idx_using_dir = idx("using_dir");
        let idx_class_name = idx("class_name");
        let idx_class_decl = idx("class_decl");
        let idx_interface_name = idx("interface_name");
        let idx_interface_decl = idx("interface_decl");
        let idx_struct_name = idx("struct_name");
        let idx_record_name = idx("record_name");
        let idx_record_decl = idx("record_decl");
        let idx_enum_name = idx("enum_name");
        let idx_method_name = idx("method_name");
        let idx_constructor_name = idx("constructor_name");
        let idx_property_name = idx("property_name");
        let idx_field_decl = idx("field_decl");
        let idx_event_field_decl = idx("event_field_decl");
        let idx_event_name = idx("event_name");
        let idx_delegate_name = idx("delegate_name");
        let idx_attr_name = idx("attr_name");

        let mut matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            // Namespace
            if let Some(cap) = find_capture(m, idx_namespace_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Package,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Using directive
            if let Some(cap) = find_capture(m, idx_using_dir) {
                if let Some((name, line)) = extract_using_name(content, &cap.node) {
                    symbols.push(ParsedSymbol {
                        name,
                        kind: SymbolKind::Import,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents: vec![],
                    });
                }
                continue;
            }

            // Class
            if let Some(cap) = find_capture(m, idx_class_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                let parents = find_capture(m, idx_class_decl)
                    .and_then(|dc| find_base_list_child(&dc.node))
                    .map(|bl| parse_base_list(content, &bl))
                    .unwrap_or_default();
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents,
                });
                continue;
            }

            // Interface
            if let Some(cap) = find_capture(m, idx_interface_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                let parents = find_capture(m, idx_interface_decl)
                    .and_then(|dc| find_base_list_child(&dc.node))
                    .map(|bl| parse_base_list(content, &bl))
                    .unwrap_or_default();
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Interface,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents,
                });
                continue;
            }

            // Struct
            if let Some(cap) = find_capture(m, idx_struct_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class, // Struct -> Class
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Record
            if let Some(cap) = find_capture(m, idx_record_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                let parents = find_capture(m, idx_record_decl)
                    .and_then(|dc| find_base_list_child(&dc.node))
                    .map(|bl| parse_base_list(content, &bl))
                    .unwrap_or_default();
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class, // Record -> Class
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents,
                });
                continue;
            }

            // Enum
            if let Some(cap) = find_capture(m, idx_enum_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Enum,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Method
            if let Some(cap) = find_capture(m, idx_method_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Constructor
            if let Some(cap) = find_capture(m, idx_constructor_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Property
            if let Some(cap) = find_capture(m, idx_property_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Property,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Field declaration (may contain const)
            if let Some(cap) = find_capture(m, idx_field_decl) {
                let fields = extract_field_info(content, &cap.node);
                for (name, line, is_const) in fields {
                    let kind = if is_const {
                        SymbolKind::Constant
                    } else {
                        SymbolKind::Property
                    };
                    symbols.push(ParsedSymbol {
                        name,
                        kind,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents: vec![],
                    });
                }
                continue;
            }

            // Event field declaration
            if let Some(cap) = find_capture(m, idx_event_field_decl) {
                let events = extract_event_field_names(content, &cap.node);
                for (name, line) in events {
                    symbols.push(ParsedSymbol {
                        name,
                        kind: SymbolKind::Property, // Event -> Property
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents: vec![],
                    });
                }
                continue;
            }

            // Event declaration (with accessors)
            if let Some(cap) = find_capture(m, idx_event_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Property, // Event -> Property
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Delegate
            if let Some(cap) = find_capture(m, idx_delegate_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::TypeAlias, // Delegate -> TypeAlias
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // Attribute
            if let Some(cap) = find_capture(m, idx_attr_name) {
                let attr_name = node_text(content, &cap.node);
                // Extract just the simple name (last component of qualified name)
                let simple_name = attr_name.rsplit('.').next().unwrap_or(attr_name);
                let line = node_line(&cap.node);
                if is_significant_attr(simple_name) {
                    symbols.push(ParsedSymbol {
                        name: format!("[{}]", simple_name),
                        kind: SymbolKind::Annotation,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents: vec![],
                    });
                }
                continue;
            }
        }

        Ok(symbols)
    }
}

/// Find a base_list child node within a declaration node
fn find_base_list_child<'a>(node: &'a tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == "base_list")
}

#[cfg(test)]
#[path = "csharp_tests.rs"]
mod tests;
