//! Tree-sitter based Java parser

use anyhow::Result;
use std::collections::HashSet;
use std::sync::LazyLock;
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator};

use super::{
    CaptureIndexer, LanguageParser, find_capture, line_text, node_line, node_text, parse_tree,
};
use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;

static JAVA_LANGUAGE: LazyLock<Language> = LazyLock::new(|| tree_sitter_java::LANGUAGE.into());

static JAVA_QUERY: LazyLock<Query> = LazyLock::new(|| {
    Query::new(&JAVA_LANGUAGE, include_str!("queries/java.scm"))
        .expect("Failed to compile Java tree-sitter query")
});

pub static JAVA_PARSER: JavaParser = JavaParser;

pub struct JavaParser;

/// Parent extraction specs: (tree-sitter child node kind, inheritance keyword).
/// "superclass" uses single-type extraction; all others use type-list extraction.
const CLASS_PARENT_SPECS: &[(&str, &str)] = &[
    ("superclass", "extends"),
    ("super_interfaces", "implements"),
];
const INTERFACE_PARENT_SPECS: &[(&str, &str)] = &[("extends_interfaces", "extends")];
const ENUM_PARENT_SPECS: &[(&str, &str)] = &[("super_interfaces", "implements")];
const RECORD_PARENT_SPECS: &[(&str, &str)] = &[("super_interfaces", "implements")];

/// Significant Java/Spring annotations to track
const SIGNIFICANT_ANNOTATIONS: &[&str] = &[
    "RestController",
    "Controller",
    "Service",
    "Repository",
    "Component",
    "Entity",
    "Table",
    "Configuration",
    "Bean",
    "GetMapping",
    "PostMapping",
    "PutMapping",
    "DeleteMapping",
    "PatchMapping",
    "RequestMapping",
    "Autowired",
    "Override",
    "Transactional",
    "SpringBootApplication",
    "EnableAutoConfiguration",
    "Test",
    "BeforeEach",
    "AfterEach",
    "BeforeAll",
    "AfterAll",
    "Inject",
    "Singleton",
    "Provides",
    "Binds",
    "Module",
    "Data",
    "Value",
    "Builder",
    "AllArgsConstructor",
    "NoArgsConstructor",
    "Getter",
    "Setter",
    "Slf4j",
    "Log4j2",
];

impl LanguageParser for JavaParser {
    fn parse_symbols(&self, content: &str) -> Result<Vec<ParsedSymbol>> {
        let tree = parse_tree(content, &JAVA_LANGUAGE)?;
        let mut symbols = Vec::new();
        let query = &*JAVA_QUERY;
        let mut cursor = QueryCursor::new();

        let idx = CaptureIndexer::new(query);

        let idx_class_name = idx.get("class_name");
        let idx_class_node = idx.get("class_node");
        let idx_interface_name = idx.get("interface_name");
        let idx_interface_node = idx.get("interface_node");
        let idx_enum_name = idx.get("enum_name");
        let idx_enum_node = idx.get("enum_node");
        let idx_method_name = idx.get("method_name");
        let idx_method_node = idx.get("method_node");
        let idx_constructor_name = idx.get("constructor_name");
        let idx_constructor_node = idx.get("constructor_node");
        let idx_field_name = idx.get("field_name");
        let idx_field_node = idx.get("field_node");
        let idx_record_name = idx.get("record_name");
        let idx_record_node = idx.get("record_node");
        let idx_annotation_name = idx.get("annotation_name");
        let idx_annotation_call_name = idx.get("annotation_call_name");

        let mut emitted: HashSet<(String, usize)> = HashSet::new();

        // Track explicitly defined methods so we can skip synthetic record accessors
        let mut explicit_methods: HashSet<String> = HashSet::new();
        // Deferred record component accessors: (name, line, signature)
        let mut pending_record_accessors: Vec<(String, usize, String)> = Vec::new();

        let mut matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            // === Classes ===
            if let Some(name_cap) = find_capture(m, idx_class_name) {
                let name = node_text(content, &name_cap.node);
                let line = node_line(&name_cap.node);
                if emitted.insert((name.to_string(), line)) {
                    let parents = find_capture(m, idx_class_node)
                        .map(|n| extract_parents(content, &n.node, CLASS_PARENT_SPECS))
                        .unwrap_or_default();
                    symbols.push(ParsedSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Class,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents,
                    });
                }
                continue;
            }

            // === Interfaces ===
            if let Some(name_cap) = find_capture(m, idx_interface_name) {
                let name = node_text(content, &name_cap.node);
                let line = node_line(&name_cap.node);
                if emitted.insert((name.to_string(), line)) {
                    let parents = find_capture(m, idx_interface_node)
                        .map(|n| extract_parents(content, &n.node, INTERFACE_PARENT_SPECS))
                        .unwrap_or_default();
                    symbols.push(ParsedSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Interface,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents,
                    });
                }
                continue;
            }

            // === Enums ===
            if let Some(name_cap) = find_capture(m, idx_enum_name) {
                let name = node_text(content, &name_cap.node);
                let line = node_line(&name_cap.node);
                if emitted.insert((name.to_string(), line)) {
                    let parents = find_capture(m, idx_enum_node)
                        .map(|n| extract_parents(content, &n.node, ENUM_PARENT_SPECS))
                        .unwrap_or_default();
                    symbols.push(ParsedSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Enum,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents,
                    });
                }
                continue;
            }

            // === Records ===
            if let Some(name_cap) = find_capture(m, idx_record_name) {
                let name = node_text(content, &name_cap.node);
                let line = node_line(&name_cap.node);
                if emitted.insert((name.to_string(), line)) {
                    let record_node = find_capture(m, idx_record_node).map(|n| n.node);
                    let parents = record_node
                        .as_ref()
                        .map(|n| extract_parents(content, n, RECORD_PARENT_SPECS))
                        .unwrap_or_default();
                    symbols.push(ParsedSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Class,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents,
                    });
                    // Extract record components from formal_parameters
                    if let Some(rn) = &record_node {
                        for (comp_name, comp_type) in extract_record_components(content, rn) {
                            let comp_line = line; // components share the record's line
                            if emitted.insert((comp_name.clone(), comp_line)) {
                                symbols.push(ParsedSymbol {
                                    name: comp_name.clone(),
                                    kind: SymbolKind::Property,
                                    line: comp_line,
                                    signature: format!("{} {}", comp_type, comp_name),
                                    parents: vec![],
                                });
                                // Queue synthetic accessor
                                pending_record_accessors.push((
                                    comp_name.clone(),
                                    comp_line,
                                    format!("public {} {}()", comp_type, comp_name),
                                ));
                            }
                        }
                    }
                }
                continue;
            }

            // === Methods (only inside class/interface/enum/record body) ===
            if let Some(name_cap) = find_capture(m, idx_method_name) {
                if let Some(node_cap) = find_capture(m, idx_method_node)
                    && is_inside_type_body(&node_cap.node)
                {
                    let name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);
                    explicit_methods.insert(name.to_string());
                    if emitted.insert((name.to_string(), line)) {
                        symbols.push(ParsedSymbol {
                            name: name.to_string(),
                            kind: SymbolKind::Function,
                            line,
                            signature: line_text(content, line).trim().to_string(),
                            parents: vec![],
                        });
                    }
                }
                continue;
            }

            // === Constructors ===
            if let Some(name_cap) = find_capture(m, idx_constructor_name) {
                if let Some(node_cap) = find_capture(m, idx_constructor_node)
                    && is_inside_type_body(&node_cap.node)
                {
                    let name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);
                    if emitted.insert((name.to_string(), line)) {
                        symbols.push(ParsedSymbol {
                            name: name.to_string(),
                            kind: SymbolKind::Function,
                            line,
                            signature: line_text(content, line).trim().to_string(),
                            parents: vec![],
                        });
                    }
                }
                continue;
            }

            // === Fields (only inside class/enum body) ===
            if let Some(name_cap) = find_capture(m, idx_field_name) {
                if let Some(node_cap) = find_capture(m, idx_field_node)
                    && is_inside_type_body(&node_cap.node)
                {
                    let name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);
                    if emitted.insert((name.to_string(), line)) {
                        symbols.push(ParsedSymbol {
                            name: name.to_string(),
                            kind: SymbolKind::Property,
                            line,
                            signature: line_text(content, line).trim().to_string(),
                            parents: vec![],
                        });
                    }
                }
                continue;
            }

            // === Marker annotations (no arguments) ===
            if let Some(name_cap) = find_capture(m, idx_annotation_name) {
                let name = node_text(content, &name_cap.node);
                if SIGNIFICANT_ANNOTATIONS.contains(&name) {
                    let line = node_line(&name_cap.node);
                    if emitted.insert((format!("@{}", name), line)) {
                        symbols.push(ParsedSymbol {
                            name: format!("@{}", name),
                            kind: SymbolKind::Annotation,
                            line,
                            signature: line_text(content, line).trim().to_string(),
                            parents: vec![],
                        });
                    }
                }
                continue;
            }

            // === Annotations with arguments ===
            if let Some(name_cap) = find_capture(m, idx_annotation_call_name) {
                let name = node_text(content, &name_cap.node);
                if SIGNIFICANT_ANNOTATIONS.contains(&name) {
                    let line = node_line(&name_cap.node);
                    if emitted.insert((format!("@{}", name), line)) {
                        symbols.push(ParsedSymbol {
                            name: format!("@{}", name),
                            kind: SymbolKind::Annotation,
                            line,
                            signature: line_text(content, line).trim().to_string(),
                            parents: vec![],
                        });
                    }
                }
                continue;
            }
        }

        // Emit synthetic record component accessors (skip if an explicit override exists)
        for (name, line, signature) in pending_record_accessors {
            if !explicit_methods.contains(&name) {
                let key = (format!("{}()", name), line);
                if emitted.insert(key) {
                    symbols.push(ParsedSymbol {
                        name: format!("{}()", name),
                        kind: SymbolKind::Function,
                        line,
                        signature,
                        parents: vec![],
                    });
                }
            }
        }

        Ok(symbols)
    }
}

/// Check if a node is inside a class_body, interface_body, enum_body, or record_body
fn is_inside_type_body(node: &tree_sitter::Node) -> bool {
    node.parent()
        .map(|p| {
            matches!(
                p.kind(),
                "class_body"
                    | "interface_body"
                    | "enum_body"
                    | "enum_body_declarations"
                    | "record_body"
            )
        })
        .unwrap_or(false)
}

/// Extract parent types from a declaration node using spec-driven rules.
/// Each spec is `(child_node_kind, inherit_kind)`. The "superclass" child kind
/// uses single-type extraction; all others use type-list extraction.
fn extract_parents(
    content: &str,
    node: &tree_sitter::Node,
    specs: &[(&str, &str)],
) -> Vec<(String, String)> {
    let mut parents = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        for &(child_kind, inherit_kind) in specs {
            if child.kind() == child_kind {
                if child_kind == "superclass" {
                    if let Some(name) = extract_type_from_parent_node(&child, content) {
                        parents.push((name, inherit_kind.to_string()));
                    }
                } else {
                    extract_type_list(&child, content, inherit_kind, &mut parents);
                }
            }
        }
    }
    parents
}

/// Extract record components as (name, type_string) pairs from a record_declaration node
fn extract_record_components(
    content: &str,
    record_node: &tree_sitter::Node,
) -> Vec<(String, String)> {
    let mut components = Vec::new();
    let mut cursor = record_node.walk();
    for child in record_node.children(&mut cursor) {
        if child.kind() == "formal_parameters" {
            let mut param_cursor = child.walk();
            for param in child.children(&mut param_cursor) {
                if param.kind() == "formal_parameter" {
                    let mut name = String::new();
                    let mut type_str = String::new();
                    let mut inner = param.walk();
                    for field in param.children(&mut inner) {
                        match field.kind() {
                            "identifier" => {
                                name = node_text(content, &field).to_string();
                            }
                            "type_identifier"
                            | "integral_type"
                            | "floating_point_type"
                            | "boolean_type"
                            | "void_type"
                            | "generic_type"
                            | "scoped_type_identifier"
                            | "array_type" => {
                                type_str = node_text(content, &field).to_string();
                            }
                            _ => {}
                        }
                    }
                    if !name.is_empty() {
                        if type_str.is_empty() {
                            type_str = "Object".to_string();
                        }
                        components.push((name, type_str));
                    }
                }
            }
        }
    }
    components
}

/// Extract a single type name from a superclass node
fn extract_type_from_parent_node(node: &tree_sitter::Node, content: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" => {
                return Some(node_text(content, &child).to_string());
            }
            "generic_type" => {
                // generic_type -> type_identifier type_arguments
                if let Some(first) = child.named_child(0)
                    && first.kind() == "type_identifier"
                {
                    return Some(node_text(content, &first).to_string());
                }
            }
            "scoped_type_identifier" => {
                // Get the last identifier (e.g., com.example.MyClass -> MyClass)
                let text = node_text(content, &child);
                if let Some(last) = text.rsplit('.').next() {
                    return Some(last.to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract types from a type_list (used in super_interfaces, extends_interfaces)
fn extract_type_list(
    node: &tree_sitter::Node,
    content: &str,
    inherit_kind: &str,
    parents: &mut Vec<(String, String)>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_list" => {
                // Recurse into type_list
                extract_type_list(&child, content, inherit_kind, parents);
            }
            "type_identifier" => {
                let name = node_text(content, &child);
                parents.push((name.to_string(), inherit_kind.to_string()));
            }
            "generic_type" => {
                if let Some(first) = child.named_child(0)
                    && first.kind() == "type_identifier"
                {
                    let name = node_text(content, &first);
                    parents.push((name.to_string(), inherit_kind.to_string()));
                }
            }
            "scoped_type_identifier" => {
                let text = node_text(content, &child);
                if let Some(last) = text.rsplit('.').next() {
                    parents.push((last.to_string(), inherit_kind.to_string()));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "java_tests.rs"]
mod tests;
