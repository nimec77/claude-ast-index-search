use super::*;

#[test]
fn test_parse_class() {
    let content = "export class UserService extends BaseService implements IUserService {\n}\n\nclass ChildClass extends ParentClass {\n}\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserService" && s.kind == SymbolKind::Class)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "ChildClass" && s.parents.iter().any(|(p, _)| p == "ParentClass"))
    );
}

#[test]
fn test_parse_interface() {
    let content = "interface User {\n    id: string;\n}\n\nexport interface IUserService extends IService {\n}\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "User" && s.kind == SymbolKind::Interface)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "IUserService" && s.kind == SymbolKind::Interface)
    );
}

#[test]
fn test_parse_type_alias() {
    let content = "type UserId = string;\nexport type UserMap = Map<string, User>;\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserId" && s.kind == SymbolKind::TypeAlias)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserMap" && s.kind == SymbolKind::TypeAlias)
    );
}

#[test]
fn test_parse_enum() {
    let content = "enum Status {\n    Active,\n    Inactive,\n}\n\nexport const enum Direction {\n    Up,\n    Down,\n}\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Status" && s.kind == SymbolKind::Enum)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Direction" && s.kind == SymbolKind::Enum)
    );
}

#[test]
fn test_parse_functions() {
    let content = "function handleRequest(req: Request): Response {\n    return new Response();\n}\n\nexport async function fetchUser(id: string): Promise<User> {\n    return fetch(`/users/${id}`);\n}\n\nconst processData = (data: Data) => {\n    return data;\n};\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "handleRequest" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "fetchUser" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "processData" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_parse_react_component() {
    let content = "const Button: React.FC<ButtonProps> = ({ children, onClick }) => {\n    return <button onClick={onClick}>{children}</button>;\n};\n\nexport function UserCard({ user }: UserCardProps) {\n    return <div>{user.name}</div>;\n}\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Button" && s.kind == SymbolKind::Class)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserCard" && s.kind == SymbolKind::Class)
    );
}

#[test]
fn test_parse_react_hooks() {
    let content = "function useAuth() {\n    const [user, setUser] = useState(null);\n    return { user };\n}\n\nexport const useCounter = () => {\n    return { count: 0 };\n};\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "useAuth" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "useCounter" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_parse_constants() {
    let content = "const API_URL = 'https://api.example.com';\nexport const MAX_RETRIES = 3;\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "API_URL" && s.kind == SymbolKind::Constant)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MAX_RETRIES" && s.kind == SymbolKind::Constant)
    );
}

#[test]
fn test_parse_namespace() {
    let content = "namespace Utils {\n    export function helper() {}\n}\n\nexport namespace Types {\n    export interface User {}\n}\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Utils" && s.kind == SymbolKind::Package)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Types" && s.kind == SymbolKind::Package)
    );
}

#[test]
fn test_parse_decorators() {
    let content = "@Controller('users')\nexport class UserController {\n    @Get(':id')\n    getUser(@Param('id') id: string) {}\n}\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "@Controller" && s.kind == SymbolKind::Annotation)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "@Get" && s.kind == SymbolKind::Annotation)
    );
}

#[test]
fn test_comments_ignored() {
    let content = "// class FakeClass {}\nclass RealClass {}\n/* function fakeFunc() {} */\nfunction realFunc() {}\n";
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(symbols.iter().any(|s| s.name == "RealClass"));
    assert!(!symbols.iter().any(|s| s.name == "FakeClass"));
    assert!(symbols.iter().any(|s| s.name == "realFunc"));
    assert!(!symbols.iter().any(|s| s.name == "fakeFunc"));
}

#[test]
fn test_parse_class_methods() {
    let content = r#"
export class UserService {
constructor(private http: HttpClient) {}
getUser(id: string): User {
    return this.http.get(id);
}
private validate(data: any): boolean {
    return true;
}
}
"#;
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "UserService" && s.kind == SymbolKind::Class)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "constructor" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "getUser" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "validate" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_parse_getters_setters() {
    let content = r#"
class Config {
get value(): string { return ''; }
set value(v: string) {}
static create(): Config { return new Config(); }
}
"#;
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "value" && s.kind == SymbolKind::Function)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "create" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_parse_class_fields() {
    let content = r#"
class User {
name: string;
readonly age: number = 0;
static count: number = 0;
}
"#;
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "name" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "age" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "count" && s.kind == SymbolKind::Property)
    );
}

#[test]
fn test_parse_abstract_methods() {
    let content = r#"
abstract class Base {
abstract process(data: string): void;
abstract get name(): string;
}
"#;
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "process" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_object_literal_methods_not_indexed() {
    let content = r#"
const obj = {
method() { return 1; },
get prop() { return 2; },
};
"#;
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        !symbols
            .iter()
            .any(|s| s.name == "method" && s.kind == SymbolKind::Function)
    );
    assert!(
        !symbols
            .iter()
            .any(|s| s.name == "prop" && s.kind == SymbolKind::Function)
    );
}

#[test]
fn test_parse_dts_ambient_declarations() {
    // .d.ts files use "declare" keyword (ambient declarations)
    let content = r#"
import type { ToasterPublicMethods } from "../types.js";
export declare function useToaster(): ToasterPublicMethods;
export declare class Theme {}
export declare interface ThemeProps {
color: string;
}
export declare type ThemeColor = "light" | "dark";
export declare enum Direction {
Up = "up",
Down = "down",
}
export declare const MAX_RETRIES: number;
export declare namespace Utils {
function helper(): void;
}
declare function internalHelper(): void;
"#;
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    // declare function
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "useToaster" && s.kind == SymbolKind::Function),
        "useToaster not found; symbols: {:?}",
        symbols
            .iter()
            .map(|s| (&s.name, &s.kind))
            .collect::<Vec<_>>()
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "internalHelper" && s.kind == SymbolKind::Function)
    );
    // declare class
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Theme" && s.kind == SymbolKind::Class)
    );
    // declare interface
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "ThemeProps" && s.kind == SymbolKind::Interface)
    );
    // declare type
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "ThemeColor" && s.kind == SymbolKind::TypeAlias)
    );
    // declare enum
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Direction" && s.kind == SymbolKind::Enum)
    );
    // declare const (ALL_CAPS)
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "MAX_RETRIES" && s.kind == SymbolKind::Constant)
    );
    // declare namespace
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Utils" && s.kind == SymbolKind::Package)
    );
}

#[test]
fn test_parse_private_class_members() {
    let content = r#"
class Foo {
#secret: string = '';
#process(): void {}
}
"#;
    let symbols = TYPESCRIPT_PARSER.parse_symbols(content).unwrap();
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "#secret" && s.kind == SymbolKind::Property)
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "#process" && s.kind == SymbolKind::Function)
    );
}
