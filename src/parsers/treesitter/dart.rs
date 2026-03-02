//! Tree-sitter based Dart parser

use anyhow::Result;
use std::sync::LazyLock;
use tree_sitter::{Language, Node};

use super::{LanguageParser, line_text, node_line, node_text, parse_tree};
use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;

#[path = "dart_error_recovery.rs"]
mod error_recovery;

static DART_LANGUAGE: LazyLock<Language> = LazyLock::new(tree_sitter_dart::language);

pub static DART_PARSER: DartParser = DartParser;

pub struct DartParser;

impl LanguageParser for DartParser {
    fn parse_symbols(&self, content: &str) -> Result<Vec<ParsedSymbol>> {
        let tree = parse_tree(content, &DART_LANGUAGE)?;
        let mut symbols = Vec::new();

        // Walk the tree manually since tree-sitter-dart 0.0.4 has limited query support
        walk_node(&tree.root_node(), content, &mut symbols);

        Ok(symbols)
    }
}

/// Recursively walk the AST and extract symbols
fn walk_node(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    match node.kind() {
        "import_or_export" => {
            extract_import(node, content, symbols);
        }
        "class_definition" => {
            extract_class(node, content, symbols);
            // Continue walking for inner declarations (methods, constructors, etc.)
            walk_class_body(node, content, symbols);
            return; // Don't recurse further, we handled it
        }
        "mixin_declaration" => {
            extract_mixin(node, content, symbols);
            walk_class_body(node, content, symbols);
            return;
        }
        "extension_declaration" => {
            extract_extension(node, content, symbols);
            walk_extension_body(node, content, symbols);
            return;
        }
        "extension_type_declaration" => {
            extract_extension_type(node, content, symbols);
            walk_class_body(node, content, symbols);
            return;
        }
        "enum_declaration" => {
            extract_enum(node, content, symbols);
            walk_class_body(node, content, symbols);
            return;
        }
        "type_alias" => {
            extract_typedef(node, content, symbols);
            return;
        }
        // Top-level functions: tree-sitter-dart 0.0.4 wraps them in lambda_expression
        "lambda_expression" => {
            if is_top_level(node) {
                extract_lambda_function(node, content, symbols);
            }
            return;
        }
        "function_signature" => {
            // Only handle top-level function signatures (without body)
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_function_signature(node, content, symbols);
            }
            return;
        }
        "getter_signature" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_getter(node, content, symbols);
            }
            return;
        }
        "setter_signature" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_setter(node, content, symbols);
            }
            return;
        }
        // Top-level variable declarations (tree-sitter-dart 0.0.4 uses local_variable_declaration)
        "local_variable_declaration" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_local_var_as_property(node, content, symbols);
            }
            return;
        }
        // Top-level variable declarations
        "initialized_identifier_list" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_top_level_vars(node, content, symbols);
            }
            return;
        }
        "static_final_declaration_list" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                extract_top_level_consts(node, content, symbols);
            }
            return;
        }
        // ERROR recovery: tree-sitter-dart 0.0.4 doesn't know Dart 3 modifiers
        // sealed/base/final class, extension type, mixin class
        "ERROR" => {
            if is_top_level(node) || is_in_top_level_wrapper(node) {
                error_recovery::try_recover_from_error(node, content, symbols);
            }
            return;
        }
        _ => {}
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(&child, content, symbols);
    }
}

/// Check if a node is a direct child of the program (top-level)
fn is_top_level(node: &Node) -> bool {
    node.parent()
        .map(|p| p.kind() == "program")
        .unwrap_or(false)
}

/// Check if a node is within a top-level unnamed wrapper (program > anonymous_node > this)
fn is_in_top_level_wrapper(node: &Node) -> bool {
    if let Some(parent) = node.parent() {
        if parent.kind() == "program" {
            return true;
        }
        // Some constructs are wrapped in an unnamed sequence node at top level
        if let Some(grandparent) = parent.parent()
            && grandparent.kind() == "program"
        {
            return true;
        }
    }
    false
}

/// Extract import/export declaration
fn extract_import(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();
    let full_text = node_text(content, node);

    // Find URI string in the import
    if let Some(uri_node) = error_recovery::find_descendant_by_kind(node, "uri") {
        let uri_text = node_text(content, &uri_node);
        // Strip quotes from the URI
        let path = uri_text.trim_matches('\'').trim_matches('"');
        // Extract short name: last segment without .dart
        let short_name = path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .trim_end_matches(".dart");

        // Check if it's an export
        let _is_export = full_text.trim_start().starts_with("export");

        symbols.push(ParsedSymbol {
            name: short_name.to_string(),
            kind: SymbolKind::Import,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract class definition
fn extract_class(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => node_text(content, &n).to_string(),
        None => return,
    };

    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    // Determine kind: check for "interface" modifier
    let full_text = node_text(content, node);
    let decl_prefix = full_text.split('{').next().unwrap_or("");

    let kind =
        if decl_prefix.contains("interface class") || decl_prefix.contains("interface  class") {
            SymbolKind::Interface
        } else {
            SymbolKind::Class
        };

    // Extract parents
    let mut parents = Vec::new();

    // superclass field
    if let Some(superclass_node) = node.child_by_field_name("superclass") {
        extract_superclass_parents(&superclass_node, content, &mut parents);
    }

    // interfaces field
    if let Some(interfaces_node) = node.child_by_field_name("interfaces") {
        extract_interfaces_parents(&interfaces_node, content, &mut parents);
    }

    symbols.push(ParsedSymbol {
        name,
        kind,
        line,
        signature: sig,
        parents,
    });
}

/// Extract parents from a superclass node
fn extract_superclass_parents(node: &Node, content: &str, parents: &mut Vec<(String, String)>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" => {
                let name = node_text(content, &child).to_string();
                let base = name.split('<').next().unwrap_or(&name).trim().to_string();
                if !base.is_empty() {
                    parents.push((base, "extends".to_string()));
                }
            }
            "mixins" => {
                extract_mixins_parents(&child, content, parents);
            }
            _ => {
                if child.kind() != "extends" && child.named_child_count() > 0 {
                    extract_type_names_from_node(&child, content, parents, "extends");
                }
            }
        }
    }
}

/// Extract parent types from a mixins node ("with" clause)
fn extract_mixins_parents(node: &Node, content: &str, parents: &mut Vec<(String, String)>) {
    extract_type_names_from_node(node, content, parents, "with");
}

/// Extract parent types from an interfaces node ("implements" clause)
fn extract_interfaces_parents(node: &Node, content: &str, parents: &mut Vec<(String, String)>) {
    extract_type_names_from_node(node, content, parents, "implements");
}

/// Recursively extract type_identifier names from a node, for a given relationship kind
fn extract_type_names_from_node(
    node: &Node,
    content: &str,
    parents: &mut Vec<(String, String)>,
    kind: &str,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" {
            let name = node_text(content, &child).to_string();
            if !name.is_empty() {
                parents.push((name, kind.to_string()));
            }
        } else if child.named_child_count() > 0 && child.kind() != "type_arguments" {
            extract_type_names_from_node(&child, content, parents, kind);
        }
    }
}

/// Extract mixin declaration
fn extract_mixin(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    // Check if this is "mixin class" (Dart 3) — tree-sitter-dart 0.0.4 parses it
    // as mixin_declaration with an ERROR child "class"
    let has_class_keyword = {
        let mut cursor = node.walk();

        node.children(&mut cursor)
            .any(|c| c.kind() == "ERROR" && node_text(content, &c).trim() == "class")
    };

    if has_class_keyword {
        // "mixin class" → treat as Class
        let name = find_mixin_name(node, content);
        if name.is_empty() {
            return;
        }

        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Class,
            line,
            signature: sig,
            parents: vec![],
        });
        return;
    }

    // Regular mixin
    let name = find_mixin_name(node, content);
    if name.is_empty() {
        return;
    }

    let mut parents = Vec::new();

    let node_text_full = node_text(content, node);
    let mut cursor = node.walk();
    let mut found_on = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "on" {
            found_on = true;
        }
        if child.kind() == "type_identifier" && found_on {
            let type_name = node_text(content, &child).to_string();
            if !type_name.is_empty() {
                parents.push((type_name, "extends".to_string()));
            }
        }
        if child.kind() == "interfaces" {
            extract_interfaces_parents(&child, content, &mut parents);
        }
    }

    // Fallback: parse from text if no parents found via tree
    if parents.is_empty() && node_text_full.contains(" on ") {
        let on_part = node_text_full.split(" on ").nth(1).unwrap_or("");
        let on_types = on_part.split("implements").next().unwrap_or(on_part);
        let on_types = on_types.split('{').next().unwrap_or(on_types);
        for t in on_types.split(',') {
            let type_name = t.trim().split('<').next().unwrap_or("").trim();
            if !type_name.is_empty() {
                parents.push((type_name.to_string(), "extends".to_string()));
            }
        }
        if let Some(impl_part) = node_text_full.split("implements").nth(1) {
            let impl_part = impl_part.split('{').next().unwrap_or(impl_part);
            for t in impl_part.split(',') {
                let type_name = t.trim().split('<').next().unwrap_or("").trim();
                if !type_name.is_empty() {
                    parents.push((type_name.to_string(), "implements".to_string()));
                }
            }
        }
    }

    symbols.push(ParsedSymbol {
        name,
        kind: SymbolKind::Interface,
        line,
        signature: sig,
        parents,
    });
}

/// Find the mixin name from a mixin_declaration node
fn find_mixin_name(node: &Node, content: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return node_text(content, &child).to_string();
        }
    }
    String::new()
}

/// Extract extension declaration
fn extract_extension(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => node_text(content, &n).to_string(),
        None => return, // Anonymous extension, skip
    };

    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let mut parents = Vec::new();

    // "on" type is the "class" field in extension_declaration
    if let Some(class_node) = node.child_by_field_name("class") {
        let on_type = if class_node.kind() == "type_identifier" {
            node_text(content, &class_node).to_string()
        } else {
            error_recovery::find_first_type_identifier(&class_node, content).unwrap_or_default()
        };
        let base = on_type
            .split('<')
            .next()
            .unwrap_or(&on_type)
            .trim()
            .to_string();
        if !base.is_empty() {
            parents.push((base, "extends".to_string()));
        }
    }

    symbols.push(ParsedSymbol {
        name,
        kind: SymbolKind::Object,
        line,
        signature: sig,
        parents,
    });
}

/// Extract extension type declaration
fn extract_extension_type(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => node_text(content, &n).to_string(),
        None => return,
    };

    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let mut parents = Vec::new();

    // interfaces
    if let Some(interfaces_node) = node.child_by_field_name("interfaces") {
        extract_interfaces_parents(&interfaces_node, content, &mut parents);
    }

    symbols.push(ParsedSymbol {
        name,
        kind: SymbolKind::Class,
        line,
        signature: sig,
        parents,
    });
}

/// Extract enum declaration
fn extract_enum(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let name = match node.child_by_field_name("name") {
        Some(n) => node_text(content, &n).to_string(),
        None => return,
    };

    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let mut parents = Vec::new();

    // Standard tree: mixins and interfaces as children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "mixins" => extract_mixins_parents(&child, content, &mut parents),
            "interfaces" => extract_interfaces_parents(&child, content, &mut parents),
            // tree-sitter-dart 0.0.4: "with"/"implements" end up in ERROR node
            "ERROR" => {
                error_recovery::extract_parents_from_error_text(&child, content, &mut parents);
            }
            _ => {}
        }
    }

    symbols.push(ParsedSymbol {
        name,
        kind: SymbolKind::Enum,
        line,
        signature: sig,
        parents,
    });
}

/// Extract typedef/type_alias
fn extract_typedef(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let name = error_recovery::find_first_type_identifier(node, content).or_else(|| {
        let text = node_text(content, node);
        let after_typedef = text.strip_prefix("typedef")?.trim();
        let name_part = after_typedef.split(['=', '(', '<']).next()?;
        let tokens: Vec<&str> = name_part.split_whitespace().collect();
        if tokens.len() >= 2 {
            Some(tokens[tokens.len() - 1].to_string())
        } else if tokens.len() == 1 {
            Some(tokens[0].to_string())
        } else {
            None
        }
    });

    if let Some(name) = name
        && !name.is_empty()
    {
        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::TypeAlias,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract a function from lambda_expression at top level.
/// tree-sitter-dart 0.0.4 wraps "void main() {}" as lambda_expression > function_signature + function_body
fn extract_lambda_function(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    // Find function_signature child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_signature" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = node_text(content, &name_node).to_string();
                let line = node_line(&child);
                let sig = line_text(content, line).trim().to_string();

                symbols.push(ParsedSymbol {
                    name,
                    kind: SymbolKind::Function,
                    line,
                    signature: sig,
                    parents: vec![],
                });
                return;
            }
            // Fallback: find identifier child
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "identifier" {
                    let name = node_text(content, &inner).to_string();
                    let line = node_line(&child);
                    let sig = line_text(content, line).trim().to_string();

                    symbols.push(ParsedSymbol {
                        name,
                        kind: SymbolKind::Function,
                        line,
                        signature: sig,
                        parents: vec![],
                    });
                    return;
                }
            }
        }
    }
}

/// Extract a function_signature at top level
fn extract_function_signature(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = node_text(content, &name_node).to_string();
        let line = node_line(node);
        let sig = line_text(content, line).trim().to_string();

        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Function,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract getter
fn extract_getter(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = node_text(content, &name_node).to_string();
        let line = node_line(node);
        let sig = line_text(content, line).trim().to_string();

        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Property,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract setter
fn extract_setter(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = node_text(content, &name_node).to_string();
        let line = node_line(node);
        let sig = line_text(content, line).trim().to_string();

        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Property,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract top-level variable from local_variable_declaration.
/// tree-sitter-dart 0.0.4 uses local_variable_declaration for top-level vars.
fn extract_local_var_as_property(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    // local_variable_declaration > initialized_variable_definition > identifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "initialized_variable_definition"
            && let Some(id) = error_recovery::find_first_identifier(&child, content)
        {
            let line = node_line(&child);
            symbols.push(ParsedSymbol {
                name: id,
                kind: SymbolKind::Property,
                line,
                signature: line_text(content, line).trim().to_string(),
                parents: vec![],
            });
        }
    }
}

/// Walk class body for methods, constructors, getters, setters
fn walk_class_body(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let body = error_recovery::find_descendant_by_kind(node, "class_body")
        .or_else(|| error_recovery::find_descendant_by_kind(node, "enum_body"));

    if let Some(body) = body {
        walk_body_declarations(&body, content, symbols);
    }
}

/// Walk extension body for methods
fn walk_extension_body(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    if let Some(body) = node.child_by_field_name("body") {
        walk_body_declarations(&body, content, symbols);
    }
}

/// Walk body for declarations (methods, constructors, getters, setters, properties)
pub(super) fn walk_body_declarations(body: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        walk_body_member(&child, content, symbols);
    }
}

/// Process a single member in a class/extension body
fn walk_body_member(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    match node.kind() {
        "declaration" => {
            extract_declaration(node, content, symbols);
        }
        "method_signature" => {
            extract_method_signature(node, content, symbols);
        }
        "function_signature" => {
            extract_function_signature(node, content, symbols);
        }
        "getter_signature" => {
            extract_getter(node, content, symbols);
        }
        "setter_signature" => {
            extract_setter(node, content, symbols);
        }
        "constructor_signature" => {
            extract_constructor(node, content, symbols);
        }
        "factory_constructor_signature" => {
            extract_factory_constructor(node, content, symbols);
        }
        "constant_constructor_signature" => {
            extract_const_constructor(node, content, symbols);
        }
        _ => {
            // Recurse one level to find declarations
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_body_member(&child, content, symbols);
            }
        }
    }
}

/// Extract declaration (wraps method_signature, variable decls, etc.)
fn extract_declaration(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_signature" => {
                extract_function_signature(&child, content, symbols);
            }
            "getter_signature" => {
                extract_getter(&child, content, symbols);
            }
            "setter_signature" => {
                extract_setter(&child, content, symbols);
            }
            "constructor_signature" => {
                extract_constructor(&child, content, symbols);
            }
            "factory_constructor_signature" => {
                extract_factory_constructor(&child, content, symbols);
            }
            "constant_constructor_signature" => {
                extract_const_constructor(&child, content, symbols);
            }
            _ => {}
        }
    }
}

/// Extract method_signature (wraps constructor_signature, function_signature, etc.)
fn extract_method_signature(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_signature" => {
                extract_function_signature(&child, content, symbols);
            }
            "getter_signature" => {
                extract_getter(&child, content, symbols);
            }
            "setter_signature" => {
                extract_setter(&child, content, symbols);
            }
            "constructor_signature" => {
                extract_constructor(&child, content, symbols);
            }
            "factory_constructor_signature" => {
                extract_factory_constructor(&child, content, symbols);
            }
            "constant_constructor_signature" => {
                extract_const_constructor(&child, content, symbols);
            }
            _ => {}
        }
    }
}

/// Extract constructor: ClassName(...) or ClassName.named(...)
fn extract_constructor(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    // Always use collect_constructor_name to get the full name (ClassName.namedPart)
    // because child_by_field_name("name") only returns the class part
    let name_text = collect_constructor_name(node, content);

    if !name_text.is_empty() {
        symbols.push(ParsedSymbol {
            name: name_text,
            kind: SymbolKind::Function,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Collect constructor name from node children (identifiers and dots joined)
fn collect_constructor_name(node: &Node, content: &str) -> String {
    let mut parts = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            parts.push(node_text(content, &child));
        }
        // Stop at formal_parameter_list (constructor args)
        if child.kind() == "formal_parameter_list" {
            break;
        }
    }
    parts.join(".")
}

/// Extract factory constructor
fn extract_factory_constructor(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let name = collect_constructor_name(node, content);

    if !name.is_empty() {
        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Function,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract const constructor
fn extract_const_constructor(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let line = node_line(node);
    let sig = line_text(content, line).trim().to_string();

    let name = collect_constructor_name(node, content);

    if !name.is_empty() {
        symbols.push(ParsedSymbol {
            name,
            kind: SymbolKind::Function,
            line,
            signature: sig,
            parents: vec![],
        });
    }
}

/// Extract top-level variable declarations (final/var/type)
fn extract_top_level_vars(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "initialized_identifier"
            && let Some(id) = error_recovery::find_first_identifier(&child, content)
        {
            let line = node_line(&child);
            symbols.push(ParsedSymbol {
                name: id,
                kind: SymbolKind::Property,
                line,
                signature: line_text(content, line).trim().to_string(),
                parents: vec![],
            });
        }
    }
}

/// Extract top-level constant declarations
fn extract_top_level_consts(node: &Node, content: &str, symbols: &mut Vec<ParsedSymbol>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "static_final_declaration"
            && let Some(id) = error_recovery::find_first_identifier(&child, content)
        {
            let line = node_line(&child);
            symbols.push(ParsedSymbol {
                name: id,
                kind: SymbolKind::Property,
                line,
                signature: line_text(content, line).trim().to_string(),
                parents: vec![],
            });
        }
    }
}

#[cfg(test)]
#[path = "dart_tests.rs"]
mod tests;
