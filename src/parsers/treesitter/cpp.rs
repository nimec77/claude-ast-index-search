//! Tree-sitter based C++ parser
//!
//! Parses C and C++ source files to extract:
//! - Classes and structs (including template classes)
//! - Functions (including template functions and JNI exports)
//! - Method definitions (ClassName::MethodName)
//! - Namespaces (including nested C++17 syntax)
//! - Enums (including enum class)
//! - Type aliases (typedef and using)
//! - Function-like macros (#define)
//! - Includes (#include)

use anyhow::Result;
use std::sync::LazyLock;
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator};

use super::{LanguageParser, find_capture, line_text, node_line, node_text, parse_tree};
use crate::db::SymbolKind;
use crate::parsers::ParsedSymbol;

static CPP_LANGUAGE: LazyLock<Language> = LazyLock::new(|| tree_sitter_cpp::LANGUAGE.into());

static CPP_QUERY: LazyLock<Query> = LazyLock::new(|| {
    Query::new(&CPP_LANGUAGE, include_str!("queries/cpp.scm"))
        .expect("Failed to compile C++ tree-sitter query")
});

pub static CPP_PARSER: CppParser = CppParser;

pub struct CppParser;

impl LanguageParser for CppParser {
    fn parse_symbols(&self, content: &str) -> Result<Vec<ParsedSymbol>> {
        let tree = parse_tree(content, &CPP_LANGUAGE)?;
        let mut symbols = Vec::new();
        let mut cursor = QueryCursor::new();
        let query = &*CPP_QUERY;

        // Build capture name -> index map
        let capture_names = query.capture_names();
        let idx = |name: &str| -> Option<u32> {
            capture_names
                .iter()
                .position(|n| *n == name)
                .map(|i| i as u32)
        };

        // Class/struct captures
        let idx_class_name = idx("class_name");
        let idx_class_node = idx("class_node");
        let idx_struct_name = idx("struct_name");
        let idx_struct_node = idx("struct_node");
        let idx_template_class_name = idx("template_class_name");
        let idx_template_class_node = idx("template_class_node");
        let idx_template_struct_name = idx("template_struct_name");
        let idx_template_struct_node = idx("template_struct_node");

        // Function captures
        let idx_func_name = idx("func_name");
        let idx_template_func_name = idx("template_func_name");
        let idx_method_class = idx("method_class");
        let idx_method_name = idx("method_name");
        let idx_template_method_class = idx("template_method_class");
        let idx_template_method_name = idx("template_method_name");
        let idx_destructor_class = idx("destructor_class");
        let idx_destructor_name = idx("destructor_name");

        // Other captures
        let idx_namespace_name = idx("namespace_name");
        let idx_enum_name = idx("enum_name");
        let idx_typedef_name = idx("typedef_name");
        let idx_typedef_node = idx("typedef_node");
        let idx_using_alias_name = idx("using_alias_name");
        let idx_macro_name = idx("macro_name");
        let idx_include_path = idx("include_path");

        let mut matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            // --- Class with body (not forward declaration) ---
            if let Some(name_cap) = find_capture(m, idx_class_name) {
                if find_capture(m, idx_class_node).is_some() {
                    let name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);
                    let parents = extract_base_classes(content, &name_cap.node);
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

            // --- Struct with body ---
            if let Some(name_cap) = find_capture(m, idx_struct_name) {
                if find_capture(m, idx_struct_node).is_some() {
                    let name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);
                    let parents = extract_base_classes(content, &name_cap.node);
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

            // --- Template class with body ---
            if let Some(name_cap) = find_capture(m, idx_template_class_name) {
                if find_capture(m, idx_template_class_node).is_some() {
                    let name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);
                    let parents = extract_base_classes(content, &name_cap.node);
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

            // --- Template struct with body ---
            if let Some(name_cap) = find_capture(m, idx_template_struct_name) {
                if find_capture(m, idx_template_struct_node).is_some() {
                    let name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);
                    let parents = extract_base_classes(content, &name_cap.node);
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

            // --- Method definition: ClassName::MethodName ---
            if let Some(class_cap) = find_capture(m, idx_method_class) {
                if let Some(name_cap) = find_capture(m, idx_method_name) {
                    let class_name = node_text(content, &class_cap.node);
                    let method_name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);

                    // Check for JNI pattern: JNIEXPORT ... JNICALL Java_pkg_Class_method
                    let sig_line = line_text(content, line).trim().to_string();
                    if is_jni_function(&sig_line) {
                        // Extract the JNI function name from the signature
                        if let Some(jni_name) = extract_jni_method_name(&sig_line) {
                            symbols.push(ParsedSymbol {
                                name: jni_name,
                                kind: SymbolKind::Function,
                                line,
                                signature: sig_line,
                                parents: vec![],
                            });
                            continue;
                        }
                    }

                    if !is_reserved_word(method_name) {
                        symbols.push(ParsedSymbol {
                            name: method_name.to_string(),
                            kind: SymbolKind::Function,
                            line,
                            signature: sig_line,
                            parents: vec![(class_name.to_string(), "member".to_string())],
                        });
                    }
                }
                continue;
            }

            // --- Template method definition: ClassName::MethodName ---
            if let Some(class_cap) = find_capture(m, idx_template_method_class) {
                if let Some(name_cap) = find_capture(m, idx_template_method_name) {
                    let class_name = node_text(content, &class_cap.node);
                    let method_name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);
                    if !is_reserved_word(method_name) {
                        symbols.push(ParsedSymbol {
                            name: method_name.to_string(),
                            kind: SymbolKind::Function,
                            line,
                            signature: line_text(content, line).trim().to_string(),
                            parents: vec![(class_name.to_string(), "member".to_string())],
                        });
                    }
                }
                continue;
            }

            // --- Destructor definition: ClassName::~ClassName ---
            if let Some(class_cap) = find_capture(m, idx_destructor_class) {
                if let Some(name_cap) = find_capture(m, idx_destructor_name) {
                    let class_name = node_text(content, &class_cap.node);
                    let dtor_name = node_text(content, &name_cap.node);
                    let line = node_line(&name_cap.node);
                    symbols.push(ParsedSymbol {
                        name: dtor_name.to_string(),
                        kind: SymbolKind::Function,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents: vec![(class_name.to_string(), "member".to_string())],
                    });
                }
                continue;
            }

            // --- Template function ---
            if let Some(cap) = find_capture(m, idx_template_func_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                if !is_reserved_word(name) {
                    symbols.push(ParsedSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        line,
                        signature: line_text(content, line).trim().to_string(),
                        parents: vec![],
                    });
                }
                continue;
            }

            // --- Regular function ---
            if let Some(cap) = find_capture(m, idx_func_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);

                // Check for JNI pattern in signature line
                let sig_line = line_text(content, line).trim().to_string();
                if is_jni_function(&sig_line)
                    && let Some(jni_name) = extract_jni_method_name(&sig_line)
                {
                    symbols.push(ParsedSymbol {
                        name: jni_name,
                        kind: SymbolKind::Function,
                        line,
                        signature: sig_line,
                        parents: vec![],
                    });
                    continue;
                }

                if !is_reserved_word(name) {
                    symbols.push(ParsedSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        line,
                        signature: sig_line,
                        parents: vec![],
                    });
                }
                continue;
            }

            // --- Namespace ---
            if let Some(cap) = find_capture(m, idx_namespace_name) {
                let full_name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                let sig = line_text(content, line).trim().to_string();

                if !full_name.is_empty() {
                    // For nested namespaces (a::b::c), emit each part and the full name
                    if full_name.contains("::") {
                        for part in full_name.split("::") {
                            if !part.is_empty() {
                                symbols.push(ParsedSymbol {
                                    name: part.to_string(),
                                    kind: SymbolKind::Package,
                                    line,
                                    signature: sig.clone(),
                                    parents: vec![],
                                });
                            }
                        }
                        symbols.push(ParsedSymbol {
                            name: full_name.to_string(),
                            kind: SymbolKind::Package,
                            line,
                            signature: sig,
                            parents: vec![],
                        });
                    } else {
                        symbols.push(ParsedSymbol {
                            name: full_name.to_string(),
                            kind: SymbolKind::Package,
                            line,
                            signature: sig,
                            parents: vec![],
                        });
                    }
                }
                continue;
            }

            // --- Enum ---
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

            // --- Typedef (simple: typedef ... Name;) ---
            if let Some(cap) = find_capture(m, idx_typedef_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::TypeAlias,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // --- Typedef (complex: function pointers, etc.) ---
            // This catches type_definition nodes not handled by the simple pattern above
            if let Some(cap) = find_capture(m, idx_typedef_node) {
                // Skip if this was already handled by the simple typedef_name capture
                if find_capture(m, idx_typedef_name).is_none() {
                    let line = node_line(&cap.node);
                    if let Some(name) = extract_typedef_name(&cap.node, content) {
                        symbols.push(ParsedSymbol {
                            name,
                            kind: SymbolKind::TypeAlias,
                            line,
                            signature: line_text(content, line).trim().to_string(),
                            parents: vec![],
                        });
                    }
                }
                continue;
            }

            // --- Using alias ---
            if let Some(cap) = find_capture(m, idx_using_alias_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::TypeAlias,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // --- Function-like macro ---
            if let Some(cap) = find_capture(m, idx_macro_name) {
                let name = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Constant,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![],
                });
                continue;
            }

            // --- Include ---
            if let Some(cap) = find_capture(m, idx_include_path) {
                let raw_path = node_text(content, &cap.node);
                let line = node_line(&cap.node);
                // Strip quotes and angle brackets
                let path = raw_path
                    .trim_matches('"')
                    .trim_start_matches('<')
                    .trim_end_matches('>');
                // Extract file name from path (last component)
                let name = path.rsplit('/').next().unwrap_or(path);
                symbols.push(ParsedSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Import,
                    line,
                    signature: line_text(content, line).trim().to_string(),
                    parents: vec![(path.to_string(), "from".to_string())],
                });
                continue;
            }
        }

        Ok(symbols)
    }
}

/// Extract the name from a complex typedef declaration.
/// For `typedef void (*Callback)(int, int);`, the name "Callback" is nested inside
/// function_declarator -> parenthesized_declarator -> pointer_declarator -> type_identifier.
/// This function recursively walks the declarator to find the identifier.
fn extract_typedef_name(type_def_node: &tree_sitter::Node, content: &str) -> Option<String> {
    // Look for the "declarator" field on the type_definition node
    let declarator = type_def_node.child_by_field_name("declarator")?;
    find_identifier_in_declarator(&declarator, content)
}

/// Recursively search a declarator subtree for the first type_identifier or identifier
fn find_identifier_in_declarator(node: &tree_sitter::Node, content: &str) -> Option<String> {
    // If this node is a type_identifier or identifier, it's our name
    if node.kind() == "type_identifier" || node.kind() == "identifier" {
        return Some(node_text(content, node).to_string());
    }

    // Recurse into children
    let mut walker = node.walk();
    for child in node.children(&mut walker) {
        if let Some(name) = find_identifier_in_declarator(&child, content) {
            return Some(name);
        }
    }
    None
}

/// Extract base class names from a class/struct specifier node.
/// Walks up to the parent (class_specifier or struct_specifier) and looks for base_class_clause.
fn extract_base_classes(content: &str, name_node: &tree_sitter::Node) -> Vec<(String, String)> {
    let mut parents = Vec::new();
    if let Some(class_node) = name_node.parent() {
        let mut walker = class_node.walk();
        for child in class_node.children(&mut walker) {
            if child.kind() == "base_class_clause" {
                let mut inner_walker = child.walk();
                for base_child in child.children(&mut inner_walker) {
                    // Look for type_identifier or template_type nodes inside base_class_clause
                    if base_child.kind() == "type_identifier" {
                        let base_name = node_text(content, &base_child);
                        parents.push((base_name.to_string(), "extends".to_string()));
                    } else if base_child.kind() == "template_type" {
                        // template_type has a name child (type_identifier)
                        let mut tt_walker = base_child.walk();
                        for tt_child in base_child.children(&mut tt_walker) {
                            if tt_child.kind() == "type_identifier" {
                                let base_name = node_text(content, &tt_child);
                                parents.push((base_name.to_string(), "extends".to_string()));
                                break;
                            }
                        }
                    } else if base_child.kind() == "qualified_identifier" {
                        let base_name = node_text(content, &base_child);
                        parents.push((base_name.to_string(), "extends".to_string()));
                    } else if base_child.kind() == "access_specifier" {
                        // Skip access specifiers (public, private, protected)
                        continue;
                    }
                }
            }
        }
    }
    parents
}

/// Check if a line looks like a JNI function declaration
fn is_jni_function(line: &str) -> bool {
    line.contains("JNIEXPORT") && line.contains("JNICALL")
}

/// Extract the method name from a JNI function (last part after last underscore in Java_... name)
fn extract_jni_method_name(line: &str) -> Option<String> {
    // Find Java_... pattern in the line
    let java_start = line.find("Java_")?;
    let rest = &line[java_start..];
    // The JNI name ends at '(' or whitespace
    let end = rest
        .find(|c: char| c == '(' || c.is_whitespace())
        .unwrap_or(rest.len());
    let jni_name = &rest[..end];
    // Method name is after the last '_'
    let method = jni_name.rsplit('_').next()?;
    if method.is_empty() {
        None
    } else {
        Some(method.to_string())
    }
}

/// Check if name is a C++ reserved word
fn is_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "if" | "else"
            | "while"
            | "for"
            | "do"
            | "switch"
            | "case"
            | "default"
            | "break"
            | "continue"
            | "return"
            | "goto"
            | "try"
            | "catch"
            | "throw"
            | "new"
            | "delete"
            | "this"
            | "sizeof"
            | "typeid"
            | "static_cast"
            | "dynamic_cast"
            | "const_cast"
            | "reinterpret_cast"
            | "nullptr"
            | "true"
            | "false"
            | "auto"
            | "register"
            | "static"
            | "extern"
            | "mutable"
            | "thread_local"
            | "inline"
            | "virtual"
            | "explicit"
            | "friend"
            | "constexpr"
            | "decltype"
            | "noexcept"
            | "override"
            | "final"
            | "public"
            | "private"
            | "protected"
            | "using"
            | "namespace"
            | "class"
            | "struct"
            | "union"
            | "enum"
            | "typedef"
            | "template"
            | "typename"
            | "concept"
            | "requires"
            | "co_await"
            | "co_return"
            | "co_yield"
            | "operator"
            | "main"
    )
}

#[cfg(test)]
#[path = "cpp_tests.rs"]
mod tests;
