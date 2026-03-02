use super::*;

// --- Classes ---

#[test]
fn test_parse_class() {
    let content = r#"
class TJavaException {
public:
TJavaException() {}
};
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Class && s.name == "TJavaException"),
        "Expected to find class TJavaException, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_class_with_inheritance() {
    let content = r#"
class TJniClass : public TJniReference {
public:
TJniClass() {}
};
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    let class = symbols
        .iter()
        .find(|s| s.name == "TJniClass")
        .expect("TJniClass not found");
    assert_eq!(class.kind, SymbolKind::Class);
    assert!(
        class.parents.iter().any(|(p, _)| p == "TJniReference"),
        "Expected parent TJniReference, got: {:?}",
        class.parents
    );
}

#[test]
fn test_parse_class_with_template_base() {
    let content = r#"
class TJniClass : public TJniReference<jclass> {
public:
TJniClass() {}
};
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    let class = symbols
        .iter()
        .find(|s| s.name == "TJniClass")
        .expect("TJniClass not found");
    assert_eq!(class.kind, SymbolKind::Class);
    assert!(
        class.parents.iter().any(|(p, _)| p == "TJniReference"),
        "Expected parent TJniReference, got: {:?}",
        class.parents
    );
}

#[test]
fn test_parse_struct() {
    let content = r#"
struct Point {
int x;
int y;
};
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Class && s.name == "Point"),
        "Expected to find struct Point as Class, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_template_class() {
    let content = r#"
template<class T>
class TJniReference : public TNonCopyable {
T value_;
public:
T Get() const { return value_; }
};
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Class && s.name == "TJniReference"),
        "Expected to find template class TJniReference, got: {:?}",
        symbols
    );
}

#[test]
fn test_skip_forward_declaration() {
    let content = r#"
class Foo;
struct Bar;
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    // Forward declarations have no body, so should not be captured
    assert!(
        !symbols
            .iter()
            .any(|s| s.name == "Foo" && s.kind == SymbolKind::Class),
        "Forward declaration class Foo should be skipped, got: {:?}",
        symbols
    );
    assert!(
        !symbols
            .iter()
            .any(|s| s.name == "Bar" && s.kind == SymbolKind::Class),
        "Forward declaration struct Bar should be skipped, got: {:?}",
        symbols
    );
}

// --- Functions ---

#[test]
fn test_parse_function() {
    let content = r#"
void doSomething(int x) {
return;
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Function && s.name == "doSomething"),
        "Expected function doSomething, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_template_function() {
    let content = r#"
template<class Func>
auto jniWrapExceptions(JNIEnv* env, Func&& func) {
try { return func(); }
catch (...) { }
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Function && s.name == "jniWrapExceptions"),
        "Expected template function jniWrapExceptions, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_method_definition() {
    let content = r#"
void MyClass::doWork(int x) {
return;
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    let method = symbols
        .iter()
        .find(|s| s.name == "doWork")
        .expect("doWork not found");
    assert_eq!(method.kind, SymbolKind::Function);
    assert!(
        method
            .parents
            .iter()
            .any(|(p, k)| p == "MyClass" && k == "member"),
        "Expected parent MyClass with role member, got: {:?}",
        method.parents
    );
}

#[test]
fn test_parse_destructor() {
    let content = r#"
MyClass::~MyClass() {
cleanup();
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols.iter().any(|s| s.name == "~MyClass"
            && s.kind == SymbolKind::Function
            && s.parents
                .iter()
                .any(|(p, k)| p == "MyClass" && k == "member")),
        "Expected destructor ~MyClass with parent MyClass, got: {:?}",
        symbols
    );
}

// --- Namespaces ---

#[test]
fn test_parse_namespace() {
    let content = r#"
namespace NDirect {
class Foo {};
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Package && s.name == "NDirect"),
        "Expected namespace NDirect, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_nested_namespace() {
    let content = r#"
namespace outer {
namespace inner {
    void foo() {}
}
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Package && s.name == "outer"),
        "Expected namespace outer, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Package && s.name == "inner"),
        "Expected namespace inner, got: {:?}",
        symbols
    );
}

// --- Enums ---

#[test]
fn test_parse_enum() {
    let content = r#"
enum Color {
RED,
GREEN,
BLUE
};
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Enum && s.name == "Color"),
        "Expected enum Color, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_enum_class() {
    let content = r#"
enum class Status {
Active,
Inactive,
Deleted
};
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Enum && s.name == "Status"),
        "Expected enum class Status, got: {:?}",
        symbols
    );
}

// --- Type aliases ---

#[test]
fn test_parse_typedef() {
    let content = r#"
typedef unsigned long ulong;
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::TypeAlias && s.name == "ulong"),
        "Expected typedef ulong, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_using_alias() {
    let content = r#"
using StringVec = std::vector<std::string>;
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::TypeAlias && s.name == "StringVec"),
        "Expected using alias StringVec, got: {:?}",
        symbols
    );
}

// --- Macros ---

#[test]
fn test_parse_function_macro() {
    let content = r#"
#define MAX(a, b) ((a) > (b) ? (a) : (b))
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Constant && s.name == "MAX"),
        "Expected macro MAX, got: {:?}",
        symbols
    );
}

// --- Includes ---

#[test]
fn test_parse_includes() {
    let content = r#"
#include <jni.h>
#include "util.h"
#include <util/generic/string.h>
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Import && s.name == "jni.h"),
        "Expected include jni.h, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Import && s.name == "util.h"),
        "Expected include util.h, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Import && s.name == "string.h"),
        "Expected include string.h (from util/generic/string.h), got: {:?}",
        symbols
    );
}

// --- Comments are ignored ---

#[test]
fn test_comments_ignored() {
    let content = r#"
// class FakeClass {};
class RealClass {
int x;
};
/* void fakeFunc() {} */
void realFunc() {
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols.iter().any(|s| s.name == "RealClass"),
        "Expected RealClass"
    );
    assert!(
        !symbols.iter().any(|s| s.name == "FakeClass"),
        "FakeClass should be ignored (in comment)"
    );
    assert!(
        symbols.iter().any(|s| s.name == "realFunc"),
        "Expected realFunc"
    );
    assert!(
        !symbols.iter().any(|s| s.name == "fakeFunc"),
        "fakeFunc should be ignored (in comment)"
    );
}

// --- Complex scenarios ---

#[test]
fn test_parse_class_with_methods_and_namespace() {
    let content = r#"
namespace mylib {

class Widget {
public:
void draw();
int size() const;
};

void Widget::draw() {
}

int Widget::size() const {
return 0;
}

} // namespace mylib
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Package && s.name == "mylib"),
        "Expected namespace mylib"
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Class && s.name == "Widget"),
        "Expected class Widget"
    );
    assert!(
        symbols.iter().any(|s| s.name == "draw"
            && s.kind == SymbolKind::Function
            && s.parents.iter().any(|(p, _)| p == "Widget")),
        "Expected method draw with parent Widget"
    );
    assert!(
        symbols.iter().any(|s| s.name == "size"
            && s.kind == SymbolKind::Function
            && s.parents.iter().any(|(p, _)| p == "Widget")),
        "Expected method size with parent Widget"
    );
}

#[test]
fn test_parse_multiple_base_classes() {
    let content = r#"
class MyClass : public Base1, public Base2 {
int x;
};
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    let class = symbols
        .iter()
        .find(|s| s.name == "MyClass")
        .expect("MyClass not found");
    assert!(
        class.parents.iter().any(|(p, _)| p == "Base1"),
        "Expected parent Base1, got: {:?}",
        class.parents
    );
    assert!(
        class.parents.iter().any(|(p, _)| p == "Base2"),
        "Expected parent Base2, got: {:?}",
        class.parents
    );
}

#[test]
fn test_parse_constexpr_function() {
    let content = r#"
constexpr int square(int x) {
return x * x;
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Function && s.name == "square"),
        "Expected constexpr function square, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_inline_function() {
    let content = r#"
inline void helper() {
return;
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Function && s.name == "helper"),
        "Expected inline function helper, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_static_function() {
    let content = r#"
static int counter() {
static int c = 0;
return ++c;
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Function && s.name == "counter"),
        "Expected static function counter, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_template_struct() {
    let content = r#"
template<typename T>
struct Optional {
T value;
bool has_value;
};
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Class && s.name == "Optional"),
        "Expected template struct Optional, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_enum_with_type() {
    let content = r#"
enum class Color : uint8_t {
Red = 0,
Green = 1,
Blue = 2
};
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Enum && s.name == "Color"),
        "Expected enum class Color with underlying type, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_typedef_function_pointer() {
    let content = r#"
typedef void (*Callback)(int, int);
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::TypeAlias && s.name == "Callback"),
        "Expected typedef function pointer Callback, got: {:?}",
        symbols
    );
}

#[test]
fn test_jni_extraction() {
    assert_eq!(
        extract_jni_method_name("JNIEXPORT jobject JNICALL Java_com_example_TextProcessor_analyze"),
        Some("analyze".to_string())
    );
}

#[test]
fn test_reserved_words_filtered() {
    assert!(is_reserved_word("if"));
    assert!(is_reserved_word("class"));
    assert!(is_reserved_word("operator"));
    assert!(!is_reserved_word("doSomething"));
    assert!(!is_reserved_word("MyClass"));
}

#[test]
fn test_parse_anonymous_namespace() {
    let content = r#"
namespace {
void internal_func() {}
}
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();
    // Anonymous namespaces have no name, so no namespace symbol emitted
    assert!(
        !symbols.iter().any(|s| s.kind == SymbolKind::Package),
        "Anonymous namespace should not emit a Package symbol, got: {:?}",
        symbols
    );
    // But the function inside should still be captured
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Function && s.name == "internal_func"),
        "Expected function internal_func inside anonymous namespace"
    );
}

#[test]
fn test_parse_complex_file() {
    let content = r#"
#include <iostream>
#include "myheader.h"

#define STRINGIFY(x) #x

namespace utils {

enum class LogLevel {
Debug,
Info,
Warning,
Error
};

class Logger {
public:
void log(LogLevel level, const char* msg);
};

void Logger::log(LogLevel level, const char* msg) {
std::cout << msg << std::endl;
}

template<typename T>
T clamp(T value, T lo, T hi) {
return value < lo ? lo : value > hi ? hi : value;
}

typedef void (*LogCallback)(const char*);
using StringRef = const std::string&;

} // namespace utils
"#;
    let symbols = CPP_PARSER.parse_symbols(content).unwrap();

    // Includes
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Import && s.name == "iostream")
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Import && s.name == "myheader.h")
    );

    // Macro
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Constant && s.name == "STRINGIFY")
    );

    // Namespace
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Package && s.name == "utils")
    );

    // Enum class
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Enum && s.name == "LogLevel")
    );

    // Class
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Class && s.name == "Logger")
    );

    // Method definition
    assert!(symbols.iter().any(|s| s.name == "log"
        && s.kind == SymbolKind::Function
        && s.parents.iter().any(|(p, _)| p == "Logger")));

    // Template function
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::Function && s.name == "clamp")
    );

    // Typedef
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::TypeAlias && s.name == "LogCallback")
    );

    // Using alias
    assert!(
        symbols
            .iter()
            .any(|s| s.kind == SymbolKind::TypeAlias && s.name == "StringRef")
    );
}
