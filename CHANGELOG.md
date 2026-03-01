# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

- **Flutter module indexing** — `ast-index modules` now lists Flutter packages by extracting the `name:` field from `pubspec.yaml`; `serde_yaml` added for YAML parsing; malformed YAML, missing `name:` fields, and empty names are silently skipped; `INSERT OR IGNORE` prevents duplicate module entries in monorepos.

## [3.22.0] - 2026-03-01

- **Flutter project detection** — `ast-index` now recognizes Flutter/Dart projects via `pubspec.yaml`; `ProjectType::Flutter` added to the enum, `detect_project_type()` returns `Flutter (Dart)` when `pubspec.yaml` is the sole marker, `find_project_root()` walks up to the directory containing `pubspec.yaml`, and `has_build_marker()` recognizes `pubspec.yaml` as a sub-project boundary in monorepos. Mixed detection (Flutter + other markers) works automatically. Three new unit tests added.

## [3.21.1] - 2026-02-28

- **CLAUDE.md** — AI guidance file with architecture overview, command reference, code style rules, and parser development guide; expanded `.gitignore` rules for build/IDE artifacts
- **Nightly toolchain** — updated Rust toolchain to nightly; refined directory traversal logic, parser implementations, and command functionalities
- **Code style cleanup** — consolidated nested `if let` / `if` conditions using `&&`; standardized guard-clause formatting across the codebase

## [3.21.0] - 2026-02-27

- **PHP support** — full tree-sitter parser for PHP: namespaces, classes (extends/implements), interfaces, traits, enums, functions, methods, constants, properties, `use` imports, trait `use`; file extensions `.php`, `.phtml`

## [3.20.0] - 2026-02-24

- **`.d.ts` indexing from `node_modules`** — Frontend projects automatically index TypeScript type declarations from dependencies; resolves pnpm symlinks safely (no `follow_links` on FUSE mounts)
- **Tree-sitter ambient declarations** — `declare function/class/interface/type/enum/const/namespace` in `.d.ts` files now parsed correctly via tree-sitter queries
- **`search` includes refs** — `search` command now searches the `refs` table, finding library-only symbols (e.g. `useToaster` from `@gravity-ui/uikit`) even when they have no local definition

## [3.19.0] - 2026-02-22

- **`query` command** — execute raw SQL against the index DB with JSON output; enables complex joins, aggregation, and negative queries in a single call (`SELECT`, `WITH`, `EXPLAIN` only — mutations blocked)
- **`db-path` command** — print SQLite database path for direct access from Python, JS, or any language with SQLite support
- **`schema` command** — show all tables with columns and row counts in JSON
- **`agrep` command** — structural code search via ast-grep (`sg`); AST pattern matching with `$NAME`/`$$$` metavariables and `--lang` filter

## [3.18.2] - 2026-02-18

- **Fix `composables` returning 0 results** — `@Composable` and `fun` are typically on separate lines in Kotlin; rewritten to two-phase approach (find files, then multi-line scan) instead of single-line grep callback
- **Fix `previews` returning 0 results** — same multi-line issue as `composables`

## [3.18.1]

- **Tree-sitter outline for all languages** — `outline` command now delegates to tree-sitter for Java, TypeScript/JavaScript, Swift, Ruby, Rust, Scala, C#, Proto, ObjC (previously only Dart used tree-sitter, others fell through to Kotlin regex)
- **Module dependencies for extra roots** — `rebuild` now merges module files from extra roots and checks them for build system markers; Maven (`pom.xml`) triggers dependency indexing
- **Fix call-tree nested generics** — regex now handles `Map<String, List<Integer>>` correctly instead of breaking on inner `>`
- **`inject` supports @Autowired** — `inject` command searches for both `@Inject` and `@Autowired` annotations (Spring DI)
- **Partial matching in `implementations`** — `implementations "Service"` now finds implementations of `UserService`, `PaymentService`, etc. via contains matching with relevance ranking
- **Overlap validation for `add-root`** — warns when adding a root inside or parent of project root; use `--force` to override

## [3.18.0] - 2026-02-18

- **Dedicated Java parser** — Java files now use `tree-sitter-java` instead of being routed through the Kotlin parser; indexes classes, interfaces, enums, methods, constructors, fields, and Spring annotations (`@RestController`, `@Service`, `@GetMapping`, etc.)
- **Maven module support** — `pom.xml` files are recognized as module descriptors; `<artifactId>` extracted as module name, `<dependency>` entries matched against local modules
- **Improved call-tree for Java** — regex patterns now detect Java-style method definitions (`void methodName(`, `String methodName(`), `this.method()` and `super.method()` call patterns
- **Updated skill documentation** — added Java/Spring examples, Maven support notes, removed incorrect wildcard syntax

## [3.17.5] - 2026-02-17

- **No marker files** — removed `.ast-index-root` marker; project root detected via existing index DB in cache (zero files in project directory)

## [3.17.4] - 2026-02-17

- **Directory-scoped search** — when running from a subdirectory, results are automatically limited to that subtree

## [3.17.3] - 2026-02-17

- **`--threads` / `-j` flag for rebuild** — control parallel threads (e.g. `-j 32` for network filesystems where I/O is the bottleneck)

## [3.17.2] - 2026-02-17

- **Fix FUSE hang on auto-detection** — `quick_file_count` no longer stat-s `.gitignore`/`.arcignore` per directory, which caused hangs on FUSE-mounted repos

## [3.17.1] - 2026-02-17

- **`--verbose` flag for rebuild** — detailed timing logs for every step (walk, parse, DB write, lock, modules, deps) to diagnose performance issues
- **Removed `init` command** — `rebuild` creates DB from scratch, `init` was redundant
- **SQLite concurrent safety** — `busy_timeout = 5000ms` prevents "database locked" errors; file lock prevents concurrent rebuilds on same project

## [3.17.0] - 2026-02-17

- **Auto sub-projects mode** — `rebuild` automatically switches to sub-projects indexing when directory has 65K+ source files and 2+ sub-project directories
- **`--sub-projects` flag** — explicit sub-projects mode for large monorepos, indexes each subdirectory separately into a single shared DB
- **Extended VCS support** — respects `.gitignore` and `.arcignore` in monorepos without `.git` directory

## [3.16.3] - 2026-02-17

- **FTS5 prefix search fix** — `search` no longer crashes on queries like `SlowUpstream`; prefix `*` operator now correctly placed outside FTS5 quotes
- **Extended VCS support** — `rebuild`/`search`/`grep` now respect `.gitignore` and `.arcignore` in non-git monorepos, preventing hangs on large codebases
- **Fuzzy search fix** — `--fuzzy` flag now returns all matching results (exact + prefix + contains) instead of early-returning on exact match only

## [3.16.0]

- **`restore` command** — restore index from a `.db` file: `ast-index restore /path/to/index.db`

## [3.15.0] - 2026-02-14

- **TypeScript class members** — index class methods (constructor, getters/setters, static, async), fields/properties, private `#members`, and abstract methods; object literal methods correctly excluded

## [3.14.0] - 2026-02-11

- **`map` command** — compact project overview: top directories by size with symbol kind counts; `--module` for detailed drill-down with classes and inheritance
- **`conventions` command** — auto-detect architecture patterns, frameworks, and naming conventions from indexed codebase
- **`refs` command** documented in skill

## [3.13.4] - 2026-02-11

- **Android indexing performance** — eliminate 4 redundant filesystem walks during `rebuild`; XML layout files, resource files collected in the main walk, code file usages queried from DB

## [3.13.3] - 2026-02-11

- **iOS indexing performance** — eliminate 3 redundant filesystem walks during `rebuild`; storyboard/xib files and .xcassets directories are now collected in the main walk, swift file asset usages queried from DB instead of a 4th walk

## [3.13.2] - 2026-02-11

- **Fix `rebuild` losing extra roots** — `add-root` paths are now preserved across `rebuild` (previously deleted with DB)

## [3.13.1] - 2026-02-11

- **Fix plugin skill discovery** — added `"skills"` field to `plugin.json`, fixing "Unknown skill: ast-index" error when invoking `/ast-index`

## [3.13.0] - 2026-02-11

- **Scala language support** — tree-sitter parser for class, case class, object, trait, enum (Scala 3), def, val/var, type alias, given
- **Bazel project detection** — `WORKSPACE`, `WORKSPACE.bazel`, `MODULE.bazel` as project root markers
- **4x faster rebuild on non-Android/iOS projects** — skip XML layouts, storyboards, iOS assets, CocoaPods phases when no platform markers present (309s → 83s on 83k files)
- **Git default branch detection** — correctly parse `origin/trunk`, `origin/develop` from symbolic-ref, not just main/master

## [3.12.0] - 2026-02-11

- **Tree-sitter AST parsing for 12 languages** — replaced regex-based parsers with tree-sitter for Kotlin, Java, Swift, ObjC, Python, Go, Rust, Ruby, C#, C++, Dart, Proto, and TypeScript. Parsing is now based on real ASTs instead of regex heuristics — more accurate symbol extraction, correct handling of nested constructs, and fewer false positives
- **Grouped `--help` output** — commands organized into 8 logical categories (Index Management, Search & Navigation, Module Commands, Code Patterns, Android, iOS, Perl, Project Configuration) instead of a flat alphabetical list
- **Updated project description** — "Fast code search for multi-language projects"

## [3.11.2] - 2026-02-10

- **Fix `watch` command on large projects** — switched from kqueue to FSEvents (macOS) / inotify (Linux), fixes "Too many open files" error

## [3.11.1] - 2026-02-06

- **Fix `changed` command** — auto-detect default git branch (`origin/main` or `origin/master`)
- **Fix `api` command** — accept module names with dots (e.g. `module.name` → `module/name`)
- **Updated skill docs** — added `--format json`, `unused-symbols`, `watch`, multi-root commands

## [3.11.0] - 2026-02-06

- **10x faster `unused-deps`** — replaced filesystem scanning (WalkDir + read_to_string) with index-based SQL queries to `refs` table. `core` module (225 deps) now completes in ~6s instead of 60s+ timeout
- **Fixed transitive dependency logic** — correctly checks `transitive_deps` table (api chain reachability) instead of re-scanning symbols
- **Multi-VCS support for `changed`** — auto-detects VCS, auto-selects base branch (`trunk` for arc, `origin/main` for git), normalizes `origin/` prefix
- **Removed skill copying from initialize commands** — `/initialize-*` no longer copies skill files to project directory

## [3.10.4] - 2026-02-02

- **2.6x faster indexing on large projects** — fix Dart parser allocating lines vector per class declaration

## [3.10.2] - 2026-02-02

- **Fix `changed` command** — use `merge-base` instead of direct diff to show only current branch changes
- **Multi-VCS support** — auto-detect arc vs git, use correct VCS commands

## [3.10.1] - 2026-02-02

- **Fix indexing hangs on large monorepos** — disable symlink following, add max depth limit
- **Expanded excluded directories** — added `bazel-out`, `bazel-bin`, `buck-out`, `out`, `.metals`, `.dart_tool` and more
- **Better progress reporting** — output after every chunk instead of every 4th
- **GitHub Actions release workflow** — automated builds for darwin-arm64, darwin-x86_64, linux-x86_64, windows-x86_64

## [3.10.0]

- **Dart/Flutter support** — index and search Dart/Flutter codebases
  - Classes with Dart 3 modifiers: `abstract`, `sealed`, `final`, `base`, `interface`, `mixin class`
  - Mixins: `mixin Foo on Bar`
  - Extensions and extension types (Dart 3.3)
  - Enhanced enums with `with`/`implements`
  - Functions, constructors, factory constructors
  - Getters/setters, typedefs, properties
  - Imports/exports
  - Multiline class declarations
  - File types: `.dart`
- **20 new tests** — comprehensive test coverage for Dart parser

## [3.9.3] - 2026-01-30

- **Simplified plugin installation** — `install-claude-plugin` now calls `claude plugin marketplace add` and `claude plugin install` instead of manual file copying
- **Updated README** — plugin install instructions now use official `claude plugin` CLI commands

## [3.9.2] - 2026-01-30

- **Fix OOM crashes on large projects** (70K+ files)
  - Batched indexing: parse and write to DB in chunks of 500 files instead of loading everything into memory
  - Limited rayon thread pool to max 8 threads to cap peak memory
  - Skip files > 1 MB (minified/generated code)
  - Skip lines > 2000 chars in ref parser
  - Truncate ref context to 500 chars (was unbounded — minified JS lines caused 12 GB+ databases)
  - Reduced SQLite cache from 64 MB to 8 MB
- **Hardcoded directory exclusions** — always skip `node_modules`, `__pycache__`, `build`, `dist`, `target`, `vendor`, `.gradle`, `Pods`, `DerivedData`, `.next`, `.nuxt`, `.venv`, `.cache` etc. regardless of `.gitignore`
- **New project type detection** — Frontend (`package.json`), Python (`pyproject.toml`), Go (`go.mod`), Rust (`Cargo.toml`)
- **LazyLock regex** — all 146 regex compilations cached via `std::sync::LazyLock` (was recompiling per file)

## [3.9.1]

- **Performance fix** — grep-based commands now use early termination
  - Commands like `deeplinks`, `todo`, `callers` etc. stop scanning after finding `limit` results
  - Up to 100-1000x faster on large codebases (29k files: 4-35s → 10-50ms)

## [3.9.0]

- **TypeScript/JavaScript support** — index and search web projects
  - React: components, hooks (useXxx), JSX/TSX
  - Vue: SFC script extraction, defineComponent
  - Svelte: component props extraction
  - NestJS/Angular: decorators (@Controller, @Injectable, @Component)
  - Node.js: ES modules, CommonJS
  - File types: `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`, `.vue`, `.svelte`
- **Rust support** — index and search Rust codebases
  - Structs, enums, traits, impl blocks
  - Functions, macros, type aliases
  - Derive attributes tracking
  - File types: `.rs`
- **Ruby support** — index and search Ruby/Rails codebases
  - Classes, modules, methods
  - RSpec DSL (describe, it, let, context)
  - Rails: associations, validations, scopes, callbacks
  - File types: `.rb`
- **C# support** — index and search .NET projects
  - Classes, interfaces, structs, records
  - ASP.NET: controllers, HTTP attributes
  - Unity: MonoBehaviour, SerializeField
  - File types: `.cs`
- **Explore agent** — deep code investigation with confirmations
- **Review agent** — change analysis with impact assessment
- **63 tests** — comprehensive test coverage for all parsers

## [3.8.5] - 2026-01-22

- **Documentation** — added troubleshooting section for brew install merge conflict errors

## [3.8.2]

- **Plugin improvements**
  - Added C++, Protocol Buffers, and WSDL/XSD reference documentation
  - Added "Critical Rules" section to SKILL.md for better Claude integration
  - Initialize commands now copy skill documentation to project `.claude/` directory
  - Updated plugin description to include all supported languages

## [3.8.1]

- **search command fix** — `-l/--limit` parameter now correctly limits file results
- **Content search** — `search` command now also searches file contents (not just filenames and symbols)

## [3.8.0]

- **Python support** — index and search Python codebases
  - Index: `class`, `def`, `async def`, decorators
  - Imports: `import module`, `from module import name`
  - File types: `.py`
  - `outline` and `imports` commands work with Python files
- **Go support** — index and search Go codebases
  - Index: `package`, `type struct`, `type interface`, `func`, methods with receivers
  - Imports: single imports and import blocks
  - File types: `.go`
  - `outline` and `imports` commands work with Go files
- **Performance** — `deeplinks` command 200x faster (optimized pattern)

## [3.7.0]

- **call-tree command** — show complete call hierarchy going UP (who calls the callers)
  - `ast-index call-tree "functionName" --depth 3 --limit 10`
  - Works across Kotlin, Java, Swift, Objective-C, and Perl
- **--no-ignore flag** — index gitignored directories like `build/`
  - `ast-index rebuild --no-ignore`
  - Useful for finding generated code like `BuildConfig.java`

## [3.6.0] - 2026-01-20

- **Perl support** — index and search Perl codebases
  - Index: `package`, `sub`, `use constant`, `our` variables
  - Inheritance: `use base`, `use parent`, `@ISA`
  - File types: `.pm`, `.pl`, `.t`, `.pod`
  - New commands: `perl-exports`, `perl-subs`, `perl-pod`, `perl-tests`, `perl-imports`
  - Grep commands now search Perl files: `todo`, `callers`, `deprecated`, `annotations`
  - `imports` command now parses Perl `use`/`require` statements
  - Perl packages indexed as modules for `module` command
  - Project detection: `Makefile.PL`, `Build.PL`, `cpanfile`

## [3.5.0] - 2026-01-20

- **Renamed to ast-index** — project renamed from `kotlin-index`
  - New CLI command: `ast-index` (was `kotlin-index`)
  - New Homebrew tap: `defendend/ast-index` (was `defendend/kotlin-index`)
  - New repo: `Claude-ast-index-search` (was `Claude-index-search-android-studio`)

## [3.4.1] - 2026-01-20

- **Fix grep-based commands for iOS** — 6 commands now work with Swift/ObjC:
  - `todo` — search in .swift/.m/.h files
  - `callers` — support Swift function call patterns
  - `deprecated` — support `@available(*, deprecated)` syntax
  - `annotations` — search in Swift/ObjC files (@objc, @IBAction, etc.)
  - `deeplinks` — add iOS patterns (openURL, CFBundleURLSchemes, NSUserActivity)
  - `extensions` — support Swift `extension Type` syntax

## [3.4.0] - 2026-01-20

- **iOS storyboard/xib analysis** — `storyboard-usages` command to find class usages in storyboards and xibs
- **iOS assets support** — index and search xcassets (images, colors), `asset-usages` command with `--unused` flag
- **SwiftUI commands** — `swiftui` command to find @State, @Binding, @Published, @ObservedObject properties
- **Swift concurrency** — `async-funcs` for async functions, `main-actor` for @MainActor usages
- **Combine support** — `publishers` command to find PassthroughSubject, CurrentValueSubject, AnyPublisher
- **CocoaPods/Carthage** — detect and index dependencies from Podfile and Cartfile

## [3.3.0] - 2026-01-20

- **iOS/Swift/ObjC support** — auto-detect project type and index Swift/ObjC files
- Swift: class, struct, enum, protocol, actor, extension, func, init, var/let, typealias
- ObjC: @interface, @protocol, @implementation, methods, @property, typedef, categories
- SPM module detection from Package.swift (.target, .testTarget, .binaryTarget)
- Inheritance and protocol conformance tracking for Swift/ObjC

## [3.2.0] - 2026-01-20

- Add `xml-usages` command — find class usages in XML layouts
- Add `resource-usages` command — find resource usages (drawable, string, color, etc.)
- Add `resource-usages --unused` — find unused resources in a module
- Update `unused-deps` with transitive dependency checking (via api deps)
- Update `unused-deps` with XML layout usage checking
- Update `unused-deps` with resource usage checking
- New flags: `--no-transitive`, `--no-xml`, `--no-resources`, `--strict`
- Index XML layouts (5K+ usages in large Android projects)
- Index resources (63K+ resources, 15K+ usages)
- Build transitive dependency cache (11K+ entries)

## [3.1.0] - 2026-01-20

- Add `unused-deps` command — find unused module dependencies
- Module dependencies now indexed by default (use `--no-deps` to skip)

## [3.0.0] - 2026-01-20

- **Major release** — complete Rust rewrite, replacing Python version
- 26 of 33 commands faster than Python
- Top speedups: imports (260x), dependents (100x), deps/class (90x)
- Full index with 900K+ references
- Fixed `hierarchy` multiline class declarations
- Fixed `provides` Java support and suffix matching

---

## Python versions (1.0.0 – 2.5.2)

> Legacy Python code archived in `legacy-python-mcp/` folder

### [2.5.2]

- Project-specific databases: Each project now has its own index database

### [2.5.1]

- Use ripgrep for 10-15x faster grep-based searches

### [2.5.0]

- Add `composables`, `previews`, `suspend`, `flows` commands

### [2.4.1]

- Fix `callers`, `outline`, `api` commands

### [2.4.0]

- Add `todo`, `deprecated`, `suppress`, `extensions`, `api`, `deeplinks` commands

### [2.3.0]

- Add `callers`, `imports`, `provides`, `inject` commands

### [2.2.0]

- Add `hierarchy`, `annotations`, `changed` commands

### [2.1.0]

- Fix `class` command, add `update` command

### [2.0.0]

- pip package, CLI with typer + rich, Skill for Claude Code, MCP server

### [1.2.0]

- Java support (tree-sitter-java), Find Usages, Find Implementations

### [1.1.0]

- Incremental indexing, better module detection

### [1.0.0]

- Initial release: File/symbol/module search, MCP server
