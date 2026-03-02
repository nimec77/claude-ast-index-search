use super::*;

#[test]
fn test_parse_class() {
    let content = "class MyWidget extends StatefulWidget {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let cls = symbols.iter().find(|s| s.name == "MyWidget").unwrap();
    assert_eq!(cls.kind, SymbolKind::Class);
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "StatefulWidget" && k == "extends"),
        "Expected extends StatefulWidget, got: {:?}",
        cls.parents
    );
}

#[test]
fn test_parse_abstract_class() {
    let content = "abstract class BaseService {\n  Future<void> init();\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let cls = symbols.iter().find(|s| s.name == "BaseService").unwrap();
    assert_eq!(cls.kind, SymbolKind::Class);
}

#[test]
fn test_parse_sealed_class() {
    let content = "sealed class Result {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let cls = symbols
        .iter()
        .find(|s| s.name == "Result")
        .unwrap_or_else(|| panic!("Should find sealed class Result, got: {:?}", symbols));
    assert_eq!(cls.kind, SymbolKind::Class);
}

#[test]
fn test_parse_abstract_interface_class() {
    let content = "abstract interface class AppScope {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let cls = symbols.iter().find(|s| s.name == "AppScope").unwrap();
    assert_eq!(
        cls.kind,
        SymbolKind::Interface,
        "abstract interface class should be Interface, got: {:?}",
        cls.kind
    );
}

#[test]
fn test_parse_class_with_parents() {
    let content =
        "class ApiService extends BaseService with LoggerMixin implements Disposable {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let cls = symbols
        .iter()
        .find(|s| s.name == "ApiService" && s.kind == SymbolKind::Class)
        .unwrap();
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "BaseService" && k == "extends"),
        "Expected extends BaseService, got: {:?}",
        cls.parents
    );
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "LoggerMixin" && k == "with"),
        "Expected with LoggerMixin, got: {:?}",
        cls.parents
    );
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "Disposable" && k == "implements"),
        "Expected implements Disposable, got: {:?}",
        cls.parents
    );
}

#[test]
fn test_parse_mixin() {
    let content = "mixin LoggerMixin on Object {\n  void log(String msg) {}\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let m = symbols.iter().find(|s| s.name == "LoggerMixin").unwrap();
    assert_eq!(m.kind, SymbolKind::Interface);
    assert!(
        m.parents
            .iter()
            .any(|(p, k)| p == "Object" && k == "extends"),
        "Expected extends Object, got: {:?}",
        m.parents
    );
}

#[test]
fn test_parse_mixin_with_implements() {
    let content = "mixin _PublicAppScopeImpl on _AppScopeDeps implements AppScope {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let m = symbols
        .iter()
        .find(|s| s.name == "_PublicAppScopeImpl")
        .unwrap();
    assert_eq!(m.kind, SymbolKind::Interface);
    assert!(
        m.parents
            .iter()
            .any(|(p, k)| p == "_AppScopeDeps" && k == "extends"),
        "should have _AppScopeDeps as extends parent, got: {:?}",
        m.parents
    );
    assert!(
        m.parents
            .iter()
            .any(|(p, k)| p == "AppScope" && k == "implements"),
        "should have AppScope as implements parent, got: {:?}",
        m.parents
    );
}

#[test]
fn test_parse_extension() {
    let content = "extension DateTimeX on DateTime {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let ext = symbols.iter().find(|s| s.name == "DateTimeX").unwrap();
    assert_eq!(ext.kind, SymbolKind::Object);
    assert!(
        ext.parents
            .iter()
            .any(|(p, k)| p == "DateTime" && k == "extends"),
        "Expected extends DateTime, got: {:?}",
        ext.parents
    );
}

#[test]
fn test_parse_extension_type() {
    let content = "extension type UserId(int id) implements int {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let et = symbols
        .iter()
        .find(|s| s.name == "UserId")
        .unwrap_or_else(|| panic!("Should find extension type UserId, got: {:?}", symbols));
    assert_eq!(et.kind, SymbolKind::Class);
    assert!(
        et.parents.iter().any(|(p, _)| p == "int"),
        "Expected implements int, got: {:?}",
        et.parents
    );
}

#[test]
fn test_parse_enum() {
    let content = "enum Status {\n  loading,\n  success,\n  error,\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let e = symbols.iter().find(|s| s.name == "Status").unwrap();
    assert_eq!(e.kind, SymbolKind::Enum);
}

#[test]
fn test_parse_enum_with_parents() {
    let content = "enum EnhancedEnum with Mixin implements Interface {\n  value1,\n  value2;\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let e = symbols.iter().find(|s| s.name == "EnhancedEnum").unwrap();
    assert_eq!(e.kind, SymbolKind::Enum);
    assert!(
        e.parents.iter().any(|(p, k)| p == "Mixin" && k == "with"),
        "Expected with Mixin, got: {:?}",
        e.parents
    );
    assert!(
        e.parents
            .iter()
            .any(|(p, k)| p == "Interface" && k == "implements"),
        "Expected implements Interface, got: {:?}",
        e.parents
    );
}

#[test]
fn test_parse_typedef() {
    let content = "typedef JsonMap = Map<String, dynamic>;\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let td = symbols.iter().find(|s| s.name == "JsonMap").unwrap();
    assert_eq!(td.kind, SymbolKind::TypeAlias);
}

#[test]
fn test_parse_typedef_callback() {
    let content = "typedef VoidCallback = void Function();\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let td = symbols.iter().find(|s| s.name == "VoidCallback").unwrap();
    assert_eq!(td.kind, SymbolKind::TypeAlias);
}

#[test]
fn test_parse_function() {
    let content = "void main() {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "main" && s.kind == SymbolKind::Function),
        "Should find main function, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_async_function() {
    let content = "Future<int> fetchData() async {\n  return 0;\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "fetchData" && s.kind == SymbolKind::Function),
        "Should find fetchData function, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_arrow_function() {
    let content = "String formatName(String first, String last) => '$first $last';\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "formatName" && s.kind == SymbolKind::Function),
        "Should find formatName function, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_getter_setter() {
    let content = r#"class Foo {
  int get count => _count;
  set count(int value) {
_count = value;
  }
}
"#;
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let getters: Vec<_> = symbols
        .iter()
        .filter(|s| s.name == "count" && s.kind == SymbolKind::Property)
        .collect();
    assert!(
        !getters.is_empty(),
        "should find getter 'count', got: {:?}",
        symbols
    );
    let setters: Vec<_> = symbols
        .iter()
        .filter(|s| {
            s.name == "count" && s.kind == SymbolKind::Property && s.signature.contains("set ")
        })
        .collect();
    assert!(
        !setters.is_empty(),
        "should find setter 'count', got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_constructor() {
    let content = r#"class MyService {
  MyService(this._dep);
  MyService.fromJson(Map<String, dynamic> json) {}
  factory MyService.create() => MyService(Dep());
}
"#;
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MyService" && s.kind == SymbolKind::Class),
        "Should find class MyService, got: {:?}",
        symbols
    );
    // Named constructors
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MyService.fromJson" && s.kind == SymbolKind::Function),
        "Should find MyService.fromJson constructor, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MyService.create" && s.kind == SymbolKind::Function),
        "Should find MyService.create factory constructor, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_import() {
    let content = "import 'package:flutter/material.dart';\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "material" && s.kind == SymbolKind::Import),
        "Should find import 'material', got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_export() {
    let content = "export 'src/my_widget.dart';\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "my_widget" && s.kind == SymbolKind::Import),
        "Should find export 'my_widget', got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_dart_async_import() {
    let content = "import 'dart:async';\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "dart:async" && s.kind == SymbolKind::Import),
        "Should find import 'dart:async', got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_property() {
    let content = "final String appName = 'MyApp';\nconst int maxRetries = 3;\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "appName" && s.kind == SymbolKind::Property),
        "Should find property appName, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "maxRetries" && s.kind == SymbolKind::Property),
        "Should find property maxRetries, got: {:?}",
        symbols
    );
}

#[test]
fn test_comments_ignored() {
    let content = r#"
// class FakeClass {
/* class AnotherFake { */
class RealClass {
}
"#;
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        !symbols.iter().any(|s| s.name == "FakeClass"),
        "Should not find FakeClass in comments"
    );
    assert!(
        !symbols.iter().any(|s| s.name == "AnotherFake"),
        "Should not find AnotherFake in comments"
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "RealClass" && s.kind == SymbolKind::Class),
        "Should find RealClass, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_method_inside_class() {
    let content = r#"class ApiService {
  Future<void> init() async {}
  void doSomething() {}
}
"#;
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "init" && s.kind == SymbolKind::Function),
        "Should find method init, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "doSomething" && s.kind == SymbolKind::Function),
        "Should find method doSomething, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_method_inside_extension() {
    let content = r#"extension ApiServiceX on ApiService {
  void ping() {}
}
"#;
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "ApiServiceX" && s.kind == SymbolKind::Object),
        "Should find extension ApiServiceX, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "ping" && s.kind == SymbolKind::Function),
        "Should find method ping, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_method_inside_mixin() {
    let content = r#"mixin LoggerMixin on Object {
  void log(String msg) {}
}
"#;
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "LoggerMixin" && s.kind == SymbolKind::Interface),
        "Should find mixin LoggerMixin, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "log" && s.kind == SymbolKind::Function),
        "Should find method log, got: {:?}",
        symbols
    );
}

#[test]
fn test_full_dart_file() {
    let content = r#"
import 'package:flutter/material.dart';
import 'dart:async';

typedef JsonMap = Map<String, dynamic>;

const String appVersion = '1.0.0';

mixin LoggerMixin on Object {
  void log(String msg) {}
}

abstract class BaseService {
  Future<void> init();
}

class ApiService extends BaseService with LoggerMixin implements Disposable {
  final String baseUrl;

  ApiService(this.baseUrl);

  ApiService.withDefault() : baseUrl = 'https://api.example.com';

  factory ApiService.create() => ApiService.withDefault();

  Future<void> init() async {}

  String get endpoint => '$baseUrl/v1';

  set timeout(int value) {}
}

extension ApiServiceX on ApiService {
  void ping() {}
}

enum Status {
  loading,
  success,
  error,
}
"#;
    let symbols = DART_PARSER.parse_symbols(content).unwrap();

    // Imports
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "material" && s.kind == SymbolKind::Import),
        "Should find import 'material', got: {:?}",
        symbols
    );

    // Typedef
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "JsonMap" && s.kind == SymbolKind::TypeAlias),
        "Should find typedef JsonMap, got: {:?}",
        symbols
    );

    // Property
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "appVersion" && s.kind == SymbolKind::Property),
        "Should find property appVersion, got: {:?}",
        symbols
    );

    // Mixin
    let mixin = symbols.iter().find(|s| s.name == "LoggerMixin").unwrap();
    assert_eq!(mixin.kind, SymbolKind::Interface);

    // Abstract class
    let base = symbols.iter().find(|s| s.name == "BaseService").unwrap();
    assert_eq!(base.kind, SymbolKind::Class);

    // Class with full inheritance
    let api = symbols
        .iter()
        .find(|s| s.name == "ApiService" && s.kind == SymbolKind::Class)
        .unwrap();
    assert!(
        api.parents
            .iter()
            .any(|(p, k)| p == "BaseService" && k == "extends"),
        "Expected extends BaseService, got: {:?}",
        api.parents
    );
    assert!(
        api.parents
            .iter()
            .any(|(p, k)| p == "LoggerMixin" && k == "with"),
        "Expected with LoggerMixin, got: {:?}",
        api.parents
    );
    assert!(
        api.parents
            .iter()
            .any(|(p, k)| p == "Disposable" && k == "implements"),
        "Expected implements Disposable, got: {:?}",
        api.parents
    );

    // Constructors
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "ApiService.withDefault" && s.kind == SymbolKind::Function),
        "Should find constructor ApiService.withDefault, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "ApiService.create" && s.kind == SymbolKind::Function),
        "Should find factory ApiService.create, got: {:?}",
        symbols
    );

    // Getter/Setter
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "endpoint" && s.kind == SymbolKind::Property),
        "Should find getter endpoint, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "timeout" && s.kind == SymbolKind::Property),
        "Should find setter timeout, got: {:?}",
        symbols
    );

    // Extension
    let ext = symbols.iter().find(|s| s.name == "ApiServiceX").unwrap();
    assert_eq!(ext.kind, SymbolKind::Object);
    assert!(
        ext.parents
            .iter()
            .any(|(p, k)| p == "ApiService" && k == "extends"),
        "Expected extends ApiService, got: {:?}",
        ext.parents
    );

    // Enum
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Status" && s.kind == SymbolKind::Enum),
        "Should find enum Status, got: {:?}",
        symbols
    );

    // Function inside class
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "init" && s.kind == SymbolKind::Function),
        "Should find method init, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_class_with_generics() {
    let content = "class Repository<T extends Model> implements BaseRepo<T> {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let cls = symbols
        .iter()
        .find(|s| s.name == "Repository" && s.kind == SymbolKind::Class)
        .unwrap();
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "BaseRepo" && k == "implements"),
        "Expected implements BaseRepo, got: {:?}",
        cls.parents
    );
}

#[test]
fn test_parse_base_class() {
    let content = "base class BaseModel {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let cls = symbols
        .iter()
        .find(|s| s.name == "BaseModel")
        .unwrap_or_else(|| panic!("Should find base class BaseModel, got: {:?}", symbols));
    assert_eq!(cls.kind, SymbolKind::Class);
}

#[test]
fn test_parse_final_class() {
    let content = "final class FinalModel {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let cls = symbols
        .iter()
        .find(|s| s.name == "FinalModel")
        .unwrap_or_else(|| panic!("Should find final class FinalModel, got: {:?}", symbols));
    assert_eq!(cls.kind, SymbolKind::Class);
}

#[test]
fn test_parse_mixin_class() {
    let content = "mixin class MixinClass {\n}\n";
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let cls = symbols
        .iter()
        .find(|s| s.name == "MixinClass")
        .unwrap_or_else(|| panic!("Should find mixin class MixinClass, got: {:?}", symbols));
    assert_eq!(cls.kind, SymbolKind::Class);
}

#[test]
fn test_parse_multiple_imports() {
    let content = r#"
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
export 'src/utils.dart';
"#;
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "material" && s.kind == SymbolKind::Import),
        "Should find material import, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "provider" && s.kind == SymbolKind::Import),
        "Should find provider import, got: {:?}",
        symbols
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "utils" && s.kind == SymbolKind::Import),
        "Should find utils export, got: {:?}",
        symbols
    );
}

#[test]
fn test_parse_class_multiline() {
    let content = r#"class _AppScopeContainer extends AppScopeContainer
with _AppScopeDeps, _AppScopeInitializeQueue, _PublicAppScopeImpl {
}
"#;
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    let cls = symbols
        .iter()
        .find(|s| s.name == "_AppScopeContainer" && s.kind == SymbolKind::Class)
        .unwrap();
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "AppScopeContainer" && k == "extends"),
        "should have AppScopeContainer as extends, got: {:?}",
        cls.parents
    );
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "_AppScopeDeps" && k == "with"),
        "should have _AppScopeDeps as with, got: {:?}",
        cls.parents
    );
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "_AppScopeInitializeQueue" && k == "with"),
        "should have _AppScopeInitializeQueue as with, got: {:?}",
        cls.parents
    );
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "_PublicAppScopeImpl" && k == "with"),
        "should have _PublicAppScopeImpl as with, got: {:?}",
        cls.parents
    );
}

#[test]
fn test_parse_top_level_getter_setter() {
    let content = r#"
String get appName => 'MyApp';
set appName(String value) {}
"#;
    let symbols = DART_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "appName" && s.kind == SymbolKind::Property),
        "Should find top-level getter appName, got: {:?}",
        symbols
    );
}
