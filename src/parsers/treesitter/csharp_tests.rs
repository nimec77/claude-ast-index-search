use super::*;

#[test]
fn test_parse_namespace() {
    let content = r#"namespace MyApp.Models
{
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MyApp.Models" && s.kind == SymbolKind::Package)
    );
}

#[test]
fn test_parse_file_scoped_namespace() {
    let content = "namespace MyApp.Services;\n";
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MyApp.Services" && s.kind == SymbolKind::Package)
    );
}

#[test]
fn test_parse_using() {
    let content = r#"using System;
using System.Collections.Generic;
using System.Linq;
using MyApp.Models;
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "System" && s.kind == SymbolKind::Import)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "System.Collections.Generic" && s.kind == SymbolKind::Import)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "System.Linq" && s.kind == SymbolKind::Import)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MyApp.Models" && s.kind == SymbolKind::Import)
    );
}

#[test]
fn test_parse_class() {
    let content = r#"namespace MyApp
{
public class User : BaseEntity, IDisposable
{
}

public abstract class BaseEntity
{
}
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MyApp" && s.kind == SymbolKind::Package)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "User" && s.kind == SymbolKind::Class)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "BaseEntity" && s.kind == SymbolKind::Class)
    );
    // Check parents
    let user = symbols
        .iter()
        .find(|s| s.name == "User" && s.kind == SymbolKind::Class)
        .unwrap();
    assert!(
        user.parents
            .iter()
            .any(|(p, k)| p == "BaseEntity" && k == "extends")
    );
    assert!(
        user.parents
            .iter()
            .any(|(p, k)| p == "IDisposable" && k == "implements")
    );
}

#[test]
fn test_parse_generic_class() {
    let content = r#"public class Repository<T> : IRepository<T> where T : class
{
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Repository" && s.kind == SymbolKind::Class)
    );
    let repo = symbols.iter().find(|s| s.name == "Repository").unwrap();
    assert!(
        repo.parents
            .iter()
            .any(|(p, k)| p == "IRepository" && k == "implements")
    );
}

#[test]
fn test_parse_interface() {
    let content = r#"public interface IRepository<T> : IDisposable
{
T GetById(int id);
void Save(T entity);
}

public interface IUserRepository : IRepository<User>
{
User FindByEmail(string email);
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "IRepository" && s.kind == SymbolKind::Interface)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "IUserRepository" && s.kind == SymbolKind::Interface)
    );
    // Interface methods
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "GetById" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Save" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "FindByEmail" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_parse_struct() {
    let content = r#"public struct Point
{
public int X;
public int Y;
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Point" && s.kind == SymbolKind::Class)
    );
}

#[test]
fn test_parse_record() {
    let content = r#"public record Person(string FirstName, string LastName);

public record Employee(string FirstName, string LastName, string Department) : Person(FirstName, LastName);

public record struct Point(int X, int Y);
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Person" && s.kind == SymbolKind::Class)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Employee" && s.kind == SymbolKind::Class)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Point" && s.kind == SymbolKind::Class)
    );
}

#[test]
fn test_parse_enum() {
    let content = r#"public enum Status
{
Active,
Inactive,
Pending
}

internal enum Priority
{
Low = 1,
Medium = 2,
High = 3
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Status" && s.kind == SymbolKind::Enum)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Priority" && s.kind == SymbolKind::Enum)
    );
}

#[test]
fn test_parse_methods() {
    let content = r#"public class UserService
{
public async Task<User> GetUserAsync(int id)
{
    return null;
}

public void SaveUser(User user)
{
}

private static bool ValidateEmail(string email)
{
    return false;
}
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "GetUserAsync" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "SaveUser" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "ValidateEmail" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_parse_constructor() {
    let content = r#"public class UserService
{
private readonly ILogger _logger;

public UserService(ILogger logger)
{
    _logger = logger;
}
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserService" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_parse_properties() {
    let content = r#"public class Config
{
public string Name { get; set; }
public int MaxRetries { get; private set; }
public required string ApiKey { get; init; }
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Name" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MaxRetries" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "ApiKey" && s.kind == SymbolKind::Property)
    );
}

#[test]
fn test_parse_fields() {
    let content = r#"public class Config
{
private readonly ILogger _logger;
private static string _connectionString;
public int Count;
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "_logger" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "_connectionString" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Count" && s.kind == SymbolKind::Property)
    );
}

#[test]
fn test_parse_const() {
    let content = r#"public class Config
{
public const int MAX_RETRIES = 5;
public const string DEFAULT_NAME = "test";
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MAX_RETRIES" && s.kind == SymbolKind::Constant)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "DEFAULT_NAME" && s.kind == SymbolKind::Constant)
    );
}

#[test]
fn test_parse_delegate() {
    let content = r#"public delegate void EventHandler(object sender, EventArgs e);
public delegate Task<T> AsyncHandler<T>(CancellationToken token);
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "EventHandler" && s.kind == SymbolKind::TypeAlias)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "AsyncHandler" && s.kind == SymbolKind::TypeAlias)
    );
}

#[test]
fn test_parse_event_field() {
    let content = r#"public class Publisher
{
public event EventHandler OnDataReceived;
public event Action<string> OnMessage;
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "OnDataReceived" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "OnMessage" && s.kind == SymbolKind::Property)
    );
}

#[test]
fn test_parse_event_with_accessors() {
    let content = r#"public class Publisher
{
public event EventHandler OnData
{
    add { }
    remove { }
}
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "OnData" && s.kind == SymbolKind::Property)
    );
}

#[test]
fn test_parse_attributes() {
    let content = r#"[ApiController]
[Route("api/[controller]")]
public class UsersController : ControllerBase
{
[HttpGet]
public IActionResult GetAll()
{
    return Ok();
}

[Authorize]
[HttpPost]
public IActionResult Create(UserDto user)
{
    return Created();
}
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[ApiController]" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[Route]" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[HttpGet]" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[Authorize]" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[HttpPost]" && s.kind == SymbolKind::Annotation)
    );
}

#[test]
fn test_parse_test_attributes() {
    let content = r#"public class UserTests
{
[Fact]
public void TestCreate()
{
}

[Theory]
public void TestValidate(string input)
{
}

[Test]
public void NUnitTest()
{
}

[TestMethod]
public void MSTestMethod()
{
}
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[Fact]" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[Theory]" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[Test]" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[TestMethod]" && s.kind == SymbolKind::Annotation)
    );
}

#[test]
fn test_comments_ignored() {
    let content = r#"// class FakeClass {}
class RealClass
{
}
/* interface IFake {} */
interface IReal
{
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(symbols.iter().any(|s| s.name == "RealClass"));
    assert!(!symbols.iter().any(|s| s.name == "FakeClass"));
    assert!(symbols.iter().any(|s| s.name == "IReal"));
    assert!(!symbols.iter().any(|s| s.name == "IFake"));
}

#[test]
fn test_class_with_multiple_interfaces() {
    let content = r#"public class Service : IService, IDisposable, IAsyncDisposable
{
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    let svc = symbols
        .iter()
        .find(|s| s.name == "Service" && s.kind == SymbolKind::Class)
        .unwrap();
    assert!(
        svc.parents
            .iter()
            .any(|(p, k)| p == "IService" && k == "implements")
    );
    assert!(
        svc.parents
            .iter()
            .any(|(p, k)| p == "IDisposable" && k == "implements")
    );
    assert!(
        svc.parents
            .iter()
            .any(|(p, k)| p == "IAsyncDisposable" && k == "implements")
    );
}

#[test]
fn test_class_extends_and_implements() {
    let content = r#"public class UserService : BaseService, IUserService
{
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    let svc = symbols.iter().find(|s| s.name == "UserService").unwrap();
    assert!(
        svc.parents
            .iter()
            .any(|(p, k)| p == "BaseService" && k == "extends")
    );
    assert!(
        svc.parents
            .iter()
            .any(|(p, k)| p == "IUserService" && k == "implements")
    );
}

#[test]
fn test_parse_complete_file() {
    let content = r#"using System;
using System.Collections.Generic;

namespace MyApp.Services
{
[ApiController]
public class UserController : ControllerBase, IDisposable
{
    private readonly ILogger _logger;
    public const int MAX_RETRIES = 3;

    public string Name { get; set; }

    public UserController(ILogger logger)
    {
        _logger = logger;
    }

    [HttpGet]
    public async Task<User> GetUser(int id)
    {
        return null;
    }

    public event EventHandler OnUserCreated;
}

public interface IUserService : IDisposable
{
    User GetById(int id);
}

public enum UserStatus
{
    Active,
    Inactive
}

public record UserDto(string Name, string Email);

public delegate void UserHandler(User user);

public struct Coordinate
{
    public double Lat;
    public double Lng;
}
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();

    // Imports
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "System" && s.kind == SymbolKind::Import)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "System.Collections.Generic" && s.kind == SymbolKind::Import)
    );

    // Namespace
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MyApp.Services" && s.kind == SymbolKind::Package)
    );

    // Attributes
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[ApiController]" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[HttpGet]" && s.kind == SymbolKind::Annotation)
    );

    // Class with parents
    let ctrl = symbols
        .iter()
        .find(|s| s.name == "UserController" && s.kind == SymbolKind::Class)
        .unwrap();
    assert!(
        ctrl.parents
            .iter()
            .any(|(p, k)| p == "ControllerBase" && k == "extends")
    );
    assert!(
        ctrl.parents
            .iter()
            .any(|(p, k)| p == "IDisposable" && k == "implements")
    );

    // Fields and constants
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "_logger" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MAX_RETRIES" && s.kind == SymbolKind::Constant)
    );

    // Properties
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Name" && s.kind == SymbolKind::Property)
    );

    // Constructor
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserController" && s.kind == SymbolKind::Function)
    );

    // Methods
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "GetUser" && s.kind == SymbolKind::Function)
    );

    // Events
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "OnUserCreated" && s.kind == SymbolKind::Property)
    );

    // Interface
    let iface = symbols
        .iter()
        .find(|s| s.name == "IUserService" && s.kind == SymbolKind::Interface)
        .unwrap();
    assert!(
        iface
            .parents
            .iter()
            .any(|(p, k)| p == "IDisposable" && k == "implements")
    );

    // Enum
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserStatus" && s.kind == SymbolKind::Enum)
    );

    // Record
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserDto" && s.kind == SymbolKind::Class)
    );

    // Delegate
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserHandler" && s.kind == SymbolKind::TypeAlias)
    );

    // Struct
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Coordinate" && s.kind == SymbolKind::Class)
    );
}

#[test]
fn test_non_significant_attributes_ignored() {
    let content = r#"[SomeCustomAttribute]
public class Foo
{
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    // Custom attributes should NOT be tracked
    assert!(!symbols.iter().any(|s| s.name == "[SomeCustomAttribute]"));
    // But the class should be
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Foo" && s.kind == SymbolKind::Class)
    );
}

#[test]
fn test_sealed_partial_class() {
    let content = r#"public sealed partial class AppSettings
{
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "AppSettings" && s.kind == SymbolKind::Class)
    );
}

#[test]
fn test_static_using() {
    let content = "using static System.Math;\n";
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "System.Math" && s.kind == SymbolKind::Import)
    );
}

#[test]
fn test_abstract_method() {
    let content = r#"public abstract class Base
{
public abstract void Process();
public virtual string GetName()
{
    return "";
}
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Base" && s.kind == SymbolKind::Class)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Process" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "GetName" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_record_with_base() {
    let content = r#"public record Employee(string Name) : Person(Name), IComparable
{
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    let emp = symbols
        .iter()
        .find(|s| s.name == "Employee" && s.kind == SymbolKind::Class)
        .unwrap();
    assert!(
        emp.parents
            .iter()
            .any(|(p, k)| p == "Person" && k == "extends")
    );
    assert!(
        emp.parents
            .iter()
            .any(|(p, k)| p == "IComparable" && k == "implements")
    );
}

#[test]
fn test_interface_extends_interface() {
    let content = r#"public interface IAdvanced : IBasic, IExtended
{
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    let iface = symbols
        .iter()
        .find(|s| s.name == "IAdvanced" && s.kind == SymbolKind::Interface)
        .unwrap();
    assert!(
        iface
            .parents
            .iter()
            .any(|(p, k)| p == "IBasic" && k == "implements")
    );
    assert!(
        iface
            .parents
            .iter()
            .any(|(p, k)| p == "IExtended" && k == "implements")
    );
}

#[test]
fn test_obsolete_attribute() {
    let content = r#"public class MyClass
{
[Obsolete("Use NewMethod instead")]
public void OldMethod()
{
}
}
"#;
    let symbols = CSHARP_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "[Obsolete]" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "OldMethod" && s.kind == SymbolKind::Function)
    );
}
