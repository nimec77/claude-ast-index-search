use super::*;

#[test]
fn test_parse_class() {
    let content = "public class UserService {\n}\n";
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserService" && s.kind == SymbolKind::Class)
    );
}

#[test]
fn test_parse_class_with_extends() {
    let content =
        "public class UserController extends BaseController implements Serializable {\n}\n";
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    let cls = symbols.iter().find(|s| s.name == "UserController").unwrap();
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "BaseController" && k == "extends")
    );
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "Serializable" && k == "implements")
    );
}

#[test]
fn test_parse_interface() {
    let content = "public interface UserRepository extends JpaRepository {\n    User findByName(String name);\n}\n";
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    let iface = symbols.iter().find(|s| s.name == "UserRepository").unwrap();
    assert_eq!(iface.kind, SymbolKind::Interface);
    assert!(
        iface
            .parents
            .iter()
            .any(|(p, k)| p == "JpaRepository" && k == "extends")
    );
}

#[test]
fn test_parse_enum() {
    let content = "public enum Status {\n    ACTIVE,\n    INACTIVE;\n}\n";
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Status" && s.kind == SymbolKind::Enum)
    );
}

#[test]
fn test_parse_methods() {
    let content = r#"public class UserService {
    public List<User> getUsers() { return null; }
    private void validate(User user) {}
    protected String format(String input) { return input; }
}
"#;
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "getUsers" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "validate" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "format" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_parse_constructor() {
    let content = r#"public class User {
    private String name;
    public User(String name) {
        this.name = name;
    }
}
"#;
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "User" && s.kind == SymbolKind::Class)
    );
    // Constructor is indexed as a function with the class name
    assert!(symbols.iter().filter(|s| s.name == "User").count() >= 2);
}

#[test]
fn test_parse_fields() {
    let content = r#"public class Config {
    private String apiUrl;
    public static final int MAX_RETRIES = 3;
    protected List<String> items;
}
"#;
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "apiUrl" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MAX_RETRIES" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "items" && s.kind == SymbolKind::Property)
    );
}

#[test]
fn test_parse_annotations() {
    let content = r#"@RestController
@RequestMapping("/api")
public class UserController {
    @GetMapping("/users")
    public List<User> getUsers() { return null; }

    @Override
    public String toString() { return ""; }
}
"#;
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "@RestController" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "@RequestMapping" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "@GetMapping" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "@Override" && s.kind == SymbolKind::Annotation)
    );
}

#[test]
fn test_spring_service() {
    let content = r#"@Service
public class PaymentService {
    @Autowired
    private PaymentRepository repository;

    @Transactional
    public Payment processPayment(PaymentRequest request) {
        return repository.save(request.toPayment());
    }
}
"#;
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "@Service" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "@Autowired" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "@Transactional" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "PaymentService" && s.kind == SymbolKind::Class)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "processPayment" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "repository" && s.kind == SymbolKind::Property)
    );
}

#[test]
fn test_comments_ignored() {
    let content = "// class FakeClass {}\npublic class RealClass {}\n/* void fakeMethod() {} */\n";
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    assert!(symbols.iter().any(|s| s.name == "RealClass"));
    assert!(!symbols.iter().any(|s| s.name == "FakeClass"));
    assert!(!symbols.iter().any(|s| s.name == "fakeMethod"));
}

#[test]
fn test_nonsignificant_annotations_skipped() {
    let content = r#"@SuppressWarnings("unchecked")
public class Foo {
    @Deprecated
    public void bar() {}
}
"#;
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    // SuppressWarnings and Deprecated are not in SIGNIFICANT_ANNOTATIONS
    assert!(!symbols.iter().any(|s| s.name == "@SuppressWarnings"));
    assert!(!symbols.iter().any(|s| s.name == "@Deprecated"));
    // But class and method should still be indexed
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Foo" && s.kind == SymbolKind::Class)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "bar" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_parse_record() {
    let content = "public record Point(int x, int y) {}\n";
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    // Record emits as Class
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Point" && s.kind == SymbolKind::Class),
        "Record should be indexed as Class"
    );
    // Components emitted as Property
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "x" && s.kind == SymbolKind::Property),
        "Record component x should be Property"
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "y" && s.kind == SymbolKind::Property),
        "Record component y should be Property"
    );
    // Synthetic accessors emitted as Function
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "x()" && s.kind == SymbolKind::Function),
        "Synthetic accessor x() should be Function"
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "y()" && s.kind == SymbolKind::Function),
        "Synthetic accessor y() should be Function"
    );
}

#[test]
fn test_parse_record_with_implements() {
    let content = "public record User(String name, int age) implements Serializable {}\n";
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    let rec = symbols.iter().find(|s| s.name == "User").unwrap();
    assert_eq!(rec.kind, SymbolKind::Class);
    assert!(
        rec.parents
            .iter()
            .any(|(p, k)| p == "Serializable" && k == "implements")
    );
}

#[test]
fn test_record_explicit_accessor_override() {
    let content = r#"public record Point(int x, int y) {
    public int x() {
        return Math.abs(x);
    }
}
"#;
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    // The explicit x() method should exist
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "x" && s.kind == SymbolKind::Function),
        "Explicit x method should be indexed"
    );
    // The synthetic x() accessor should NOT exist (explicit override takes precedence)
    assert!(
        !symbols.iter().any(|s| s.name == "x()"),
        "Synthetic x() accessor should be suppressed when explicit override exists"
    );
    // But y() accessor should still be emitted
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "y()" && s.kind == SymbolKind::Function),
        "Synthetic y() accessor should still be emitted"
    );
}

#[test]
fn test_generic_class_inheritance() {
    let content =
        "public class UserRepo extends CrudRepository<User, Long> implements UserRepository {\n}\n";
    let symbols = JAVA_PARSER.parse_symbols(content).unwrap();
    let cls = symbols.iter().find(|s| s.name == "UserRepo").unwrap();
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "CrudRepository" && k == "extends")
    );
    assert!(
        cls.parents
            .iter()
            .any(|(p, k)| p == "UserRepository" && k == "implements")
    );
}
