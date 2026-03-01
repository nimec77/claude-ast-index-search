//! Code indexer: file discovery, parallel parsing, and DB population.
//!
//! Sub-modules handle specific concerns:
//! - `files`: directory walk, parallel parsing, incremental updates
//! - `modules`: Gradle/SPM/Perl/Maven/Flutter module indexing and dependencies
//! - `resources`: Android XML/resources and iOS storyboard/asset indexing
//! - `node_modules`: TypeScript declaration file indexing from node_modules

pub mod files;
pub mod modules;
pub mod node_modules;
pub mod resources;

// Re-export the full public API so callers use `crate::indexer::index_directory` etc.
#[cfg(test)]
pub(crate) use files::parse_file;
pub use files::{index_directory, index_directory_scoped, update_directory_incremental};
pub use modules::{
    collect_build_files_from_db, get_module_dependents, get_module_deps, index_module_dependencies,
    index_modules, index_modules_from_files,
};
pub use node_modules::index_node_modules_dts;
pub use resources::{
    IosAssetType, ResourceType, StoryboardUsage, XmlUsage, build_transitive_deps, index_ios_assets,
    index_ios_package_managers, index_resources, index_storyboard_usages, index_xml_usages,
};

use anyhow::Result;
use ignore::WalkBuilder;
use std::fs;
use std::path::{Path, PathBuf};

use crate::parsers::{self, ParsedRef, ParsedSymbol};

/// Maximum file size to parse (1 MB). Files larger than this are skipped.
pub const MAX_FILE_SIZE: u64 = 1_000_000;

/// Number of files to parse in each rayon parallel chunk.
pub const PARSE_CHUNK_SIZE: usize = 500;

/// Maximum directory walk depth for WalkBuilder.
pub const MAX_WALK_DEPTH: usize = 50;

/// Build a rayon thread pool, respecting the `AST_INDEX_THREADS` env var.
fn build_thread_pool() -> Result<rayon::ThreadPool> {
    let num_threads = std::env::var("AST_INDEX_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().min(8))
                .unwrap_or(4)
        });
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build thread pool: {}", e))
}

/// Sorted module lookup for efficient longest-prefix matching.
/// Entries sorted by path length descending so the longest (most specific) match is found first.
pub(crate) struct ModuleLookup {
    sorted: Vec<(String, i64)>, // (path, module_id) sorted by path length desc
}

impl ModuleLookup {
    fn from_db(conn: &rusqlite::Connection) -> Result<Self> {
        let mut stmt = conn.prepare("SELECT id, path FROM modules")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?))
        })?;
        let mut sorted: Vec<(String, i64)> = Vec::new();
        for row in rows {
            let (path, id) = row?;
            sorted.push((path, id));
        }
        sorted.sort_by_key(|b| std::cmp::Reverse(b.0.len()));
        Ok(ModuleLookup { sorted })
    }

    fn find(&self, file_path: &str) -> Option<i64> {
        self.sorted
            .iter()
            .find(|(path, _)| file_path.starts_with(path.as_str()))
            .map(|(_, id)| *id)
    }
}

/// Project type detected by markers
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectType {
    Android,  // Kotlin/Java - build.gradle.kts, settings.gradle.kts
    IOS,      // Swift/ObjC - Package.swift, *.xcodeproj
    Perl,     // Perl - .pm files, Makefile.PL, Build.PL
    Frontend, // JS/TS - package.json
    Python,   // Python - pyproject.toml, setup.py, setup.cfg
    Go,       // Go - go.mod
    Rust,     // Rust - Cargo.toml
    Bazel,    // Bazel - BUILD, WORKSPACE
    Flutter,  // Dart - pubspec.yaml
    Mixed,    // Multiple platforms present
    Unknown,
}

impl ProjectType {
    pub fn as_str(&self) -> &str {
        match self {
            ProjectType::Android => "Android (Kotlin/Java)",
            ProjectType::IOS => "iOS (Swift/ObjC)",
            ProjectType::Perl => "Perl",
            ProjectType::Frontend => "Frontend (JS/TS)",
            ProjectType::Python => "Python",
            ProjectType::Go => "Go",
            ProjectType::Rust => "Rust",
            ProjectType::Bazel => "Bazel",
            ProjectType::Flutter => "Flutter (Dart)",
            ProjectType::Mixed => "Mixed",
            ProjectType::Unknown => "Unknown",
        }
    }
}

/// Check if project has build system markers (Gradle/Maven build files)
pub fn has_android_markers(root: &Path) -> bool {
    root.join("settings.gradle.kts").exists()
        || root.join("settings.gradle").exists()
        || root.join("build.gradle.kts").exists()
        || root.join("build.gradle").exists()
        || root.join("pom.xml").exists()
}

/// Check if project has iOS markers (Xcode/SPM)
pub fn has_ios_markers(root: &Path) -> bool {
    if root.join("Package.swift").exists() {
        return true;
    }
    // Check for .xcodeproj
    fs::read_dir(root)
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "xcodeproj")
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Find immediate subdirectories that are project roots.
/// Returns list of (path, project_type) for dirs with recognized project markers.
/// If 2+ subdirs have markers, treats root as monorepo and includes ALL subdirs.
pub fn find_sub_projects(root: &Path) -> Vec<(PathBuf, ProjectType)> {
    let mut marked = Vec::new();
    let mut all_dirs = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return marked,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Skip hidden and excluded dirs
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && (name.starts_with('.') || EXCLUDED_DIRS.contains(&name))
        {
            continue;
        }
        let pt = detect_project_type(&path);
        let has_marker = pt != ProjectType::Unknown || has_build_marker(&path);
        if has_marker {
            marked.push((path.clone(), pt));
        }
        all_dirs.push((path, pt));
    }
    // If 2+ subdirs have markers → monorepo, index ALL subdirs
    let mut result = if marked.len() >= 2 { all_dirs } else { marked };
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Check if directory has any build system marker (for monorepo sub-project detection)
fn has_build_marker(path: &Path) -> bool {
    path.join("ya.make").exists()
        || path.join("Makefile").exists()
        || path.join("BUILD").exists()
        || path.join("BUILD.bazel").exists()
        || path.join("CMakeLists.txt").exists()
        || path.join("pubspec.yaml").exists()
}

/// Detect project type by looking for marker files
pub fn detect_project_type(root: &Path) -> ProjectType {
    let has_gradle = root.join("settings.gradle.kts").exists()
        || root.join("settings.gradle").exists()
        || root.join("build.gradle.kts").exists()
        || root.join("build.gradle").exists()
        || root.join("pom.xml").exists();

    let has_swift = root.join("Package.swift").exists()
        || fs::read_dir(root)
            .map(|entries| {
                entries.filter_map(|e| e.ok()).any(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "xcodeproj")
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);

    // Also check subdirectories for Package.swift (SPM structure)
    let has_swift = has_swift || {
        fs::read_dir(root)
            .map(|entries| {
                entries.filter_map(|e| e.ok()).any(|e| {
                    let path = e.path();
                    path.is_dir() && path.join("Package.swift").exists()
                })
            })
            .unwrap_or(false)
    };

    // Perl project detection: Makefile.PL, Build.PL, or .pm files in root
    let has_perl = root.join("Makefile.PL").exists()
        || root.join("Build.PL").exists()
        || root.join("cpanfile").exists()
        || fs::read_dir(root)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .any(|e| e.path().extension().map(|ext| ext == "pm").unwrap_or(false))
            })
            .unwrap_or(false);

    // Frontend (JS/TS) project detection
    let has_frontend = root.join("package.json").exists();

    // Python project detection
    let has_python = root.join("pyproject.toml").exists()
        || root.join("setup.py").exists()
        || root.join("setup.cfg").exists();

    // Go project detection
    let has_go = root.join("go.mod").exists();

    // Rust project detection
    let has_rust = root.join("Cargo.toml").exists();

    // Bazel project detection
    let has_bazel = root.join("WORKSPACE").exists()
        || root.join("WORKSPACE.bazel").exists()
        || root.join("MODULE.bazel").exists();

    // Flutter/Dart project detection
    let has_flutter = root.join("pubspec.yaml").exists();

    // Count how many platforms are detected
    let count = [
        has_gradle,
        has_swift,
        has_perl,
        has_frontend,
        has_python,
        has_go,
        has_rust,
        has_bazel,
        has_flutter,
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    if count > 1 {
        ProjectType::Mixed
    } else if has_gradle {
        ProjectType::Android
    } else if has_swift {
        ProjectType::IOS
    } else if has_perl {
        ProjectType::Perl
    } else if has_frontend {
        ProjectType::Frontend
    } else if has_python {
        ProjectType::Python
    } else if has_go {
        ProjectType::Go
    } else if has_rust {
        ProjectType::Rust
    } else if has_bazel {
        ProjectType::Bazel
    } else if has_flutter {
        ProjectType::Flutter
    } else {
        ProjectType::Unknown
    }
}

/// Parsed file data for parallel processing
pub(crate) struct ParsedFile {
    pub(crate) rel_path: String,
    pub(crate) mtime: i64,
    pub(crate) size: i64,
    pub(crate) symbols: Vec<ParsedSymbol>,
    pub(crate) refs: Vec<ParsedRef>,
}

/// Directories to always exclude from indexing (regardless of .gitignore)
const EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    "__pycache__",
    ".build",
    "build",
    "dist",
    "target",
    "vendor",
    ".gradle",
    ".idea",
    "Pods",
    "DerivedData",
    ".next",
    ".nuxt",
    ".venv",
    "venv",
    ".tox",
    "coverage",
    ".cache",
    // Build system outputs
    "out",
    "bazel-out",
    "bazel-bin",
    "bazel-genfiles",
    "bazel-testlogs",
    "buck-out",
    "_build",
    // IDE / tooling
    ".metals",
    ".bsp",
    ".dart_tool",
    // Temp / generated
    "tmp",
    "temp",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    // Other
    "_site",
    ".turbo",
    ".parcel-cache",
];

/// Check if root has a .git directory/file (false for arc/FUSE mounts)
pub fn has_git_repo(root: &Path) -> bool {
    root.join(".git").exists()
}

/// Find Arc repository root (Yandex Arcadia monorepo).
/// Searches up from root looking for .arc/HEAD, stops at $HOME.
/// Returns the arc repo root path if found.
pub fn find_arc_root(root: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir();
    let mut current = Some(root.to_path_buf());
    while let Some(dir) = current {
        if dir.join(".arc").join("HEAD").exists() {
            return Some(dir);
        }
        // Stop at $HOME to avoid confusing ~/.arc (client storage) with repo marker
        if home.as_ref().map(|h| h == &dir).unwrap_or(false) {
            break;
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Check if root is inside an Arc repository
pub fn has_arc_repo(root: &Path) -> bool {
    find_arc_root(root).is_some()
}

/// Configure arc/gitignore handling on a WalkBuilder.
///
/// When `arc_root` is set, adds `.gitignore` and `.arcignore` as custom ignore
/// filenames and registers the root `.gitignore` file from the arc repo root
/// (which may sit above the walk root).
pub fn configure_walk_ignores(builder: &mut WalkBuilder, arc_root: Option<&Path>) {
    if let Some(arc) = arc_root {
        builder.add_custom_ignore_filename(".gitignore");
        builder.add_custom_ignore_filename(".arcignore");
        let root_gitignore = arc.join(".gitignore");
        if root_gitignore.exists() {
            builder.add_ignore(root_gitignore);
        }
    }
}

/// Quickly count source files in a directory, stopping at `limit`.
/// Returns the count (capped at `limit`) — avoids full traversal for large dirs.
/// Quick file count for auto-detection threshold.
/// Intentionally skips arc/gitignore checks — this is just a rough estimate,
/// and stat-ing .gitignore on every dir is too slow on FUSE mounts.
pub fn quick_file_count(root: &Path, no_ignore: bool, limit: usize) -> usize {
    let use_git = has_git_repo(root) && !no_ignore;
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)
        .follow_links(false)
        .max_depth(Some(MAX_WALK_DEPTH))
        .git_ignore(use_git)
        .git_exclude(use_git)
        .filter_entry(|entry| !is_excluded_dir(entry));
    // No arc ignore here — quick_file_count is just a rough estimate,
    // and add_custom_ignore_filename causes stat per directory (slow on FUSE)

    let mut count = 0;
    for entry in builder.build().filter_map(|e| e.ok()) {
        if let Some(ext) = entry.path().extension().and_then(|e| e.to_str())
            && parsers::is_supported_extension(ext)
        {
            count += 1;
            if count >= limit {
                return count;
            }
        }
    }
    count
}

/// Check if a path component matches an excluded directory
pub fn is_excluded_dir(entry: &ignore::DirEntry) -> bool {
    if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
        return false;
    }
    if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
        EXCLUDED_DIRS.contains(&name)
    } else {
        false
    }
}

/// Module-related file names to collect during directory walk
fn is_module_file(name: &str) -> bool {
    name == "build.gradle"
        || name == "build.gradle.kts"
        || name == "Package.swift"
        || name.ends_with(".pm")
        || name == "pom.xml"
        || name == "pubspec.yaml"
}

/// Result of the filesystem walk in index_directory.
/// Collects all interesting paths in a single walk to avoid redundant traversals.
pub struct WalkResult {
    pub file_count: usize,
    pub module_files: Vec<PathBuf>,
    // iOS
    pub storyboard_files: Vec<PathBuf>, // .storyboard, .xib
    pub xcassets_dirs: Vec<PathBuf>,    // .xcassets directories
    // Android
    pub xml_layout_files: Vec<PathBuf>, // .xml in /res/(layout|menu|navigation)
    pub res_files: Vec<PathBuf>,        // all files under /res/
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_android_project() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("settings.gradle.kts"), "").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Android);
    }

    #[test]
    fn test_detect_ios_project() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Package.swift"), "").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::IOS);
    }

    #[test]
    fn test_detect_rust_project() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Rust);
    }

    #[test]
    fn test_detect_python_project() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Python);
    }

    #[test]
    fn test_detect_go_project() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("go.mod"), "").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Go);
    }

    #[test]
    fn test_detect_frontend_project() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Frontend);
    }

    #[test]
    fn test_detect_perl_project() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("cpanfile"), "").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Perl);
    }

    #[test]
    fn test_detect_mixed_project() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Mixed);
    }

    #[test]
    fn test_detect_unknown_project() {
        let dir = TempDir::new().unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Unknown);
    }

    #[test]
    fn test_detect_flutter_project() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pubspec.yaml"), "").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Flutter);
    }

    #[test]
    fn test_detect_mixed_flutter_and_frontend() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pubspec.yaml"), "").unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_project_type(dir.path()), ProjectType::Mixed);
    }

    #[test]
    fn test_has_build_marker_pubspec() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pubspec.yaml"), "").unwrap();
        assert!(has_build_marker(dir.path()));
    }

    #[test]
    fn test_excluded_dirs_contains_expected() {
        assert!(EXCLUDED_DIRS.contains(&"node_modules"));
        assert!(EXCLUDED_DIRS.contains(&"build"));
        assert!(EXCLUDED_DIRS.contains(&"target"));
        assert!(EXCLUDED_DIRS.contains(&"bazel-out"));
        assert!(EXCLUDED_DIRS.contains(&".gradle"));
        assert!(EXCLUDED_DIRS.contains(&"Pods"));
        assert!(EXCLUDED_DIRS.contains(&"DerivedData"));
    }

    #[test]
    fn test_parse_file_skips_large_files() {
        let dir = TempDir::new().unwrap();
        let large_file = dir.path().join("large.kt");
        let content = "a".repeat(1_100_000);
        fs::write(&large_file, &content).unwrap();

        let result = parse_file(dir.path(), &large_file).unwrap();
        assert!(result.symbols.is_empty(), "should skip large files");
        assert!(result.refs.is_empty());
    }

    #[test]
    fn test_parse_file_kotlin() {
        let dir = TempDir::new().unwrap();
        let kt_file = dir.path().join("Test.kt");
        fs::write(&kt_file, "class TestClass {\n    fun doSomething() {}\n}\n").unwrap();

        let result = parse_file(dir.path(), &kt_file).unwrap();
        assert!(result.symbols.iter().any(|s| s.name == "TestClass"));
        assert!(result.symbols.iter().any(|s| s.name == "doSomething"));
    }

    #[test]
    fn test_parse_file_swift() {
        let dir = TempDir::new().unwrap();
        let swift_file = dir.path().join("Test.swift");
        fs::write(
            &swift_file,
            "class MyView: UIView {\n    func setup() {}\n}\n",
        )
        .unwrap();

        let result = parse_file(dir.path(), &swift_file).unwrap();
        assert!(result.symbols.iter().any(|s| s.name == "MyView"));
        assert!(result.symbols.iter().any(|s| s.name == "setup"));
    }

    #[test]
    fn test_parse_file_python() {
        let dir = TempDir::new().unwrap();
        let py_file = dir.path().join("test.py");
        fs::write(
            &py_file,
            "class Service:\n    def process(self):\n        pass\n",
        )
        .unwrap();

        let result = parse_file(dir.path(), &py_file).unwrap();
        assert!(result.symbols.iter().any(|s| s.name == "Service"));
        assert!(result.symbols.iter().any(|s| s.name == "process"));
    }

    // Helper: create an in-memory SQLite DB with the modules table.
    fn make_modules_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE modules (
                id   INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL,
                kind TEXT
            );",
        )
        .unwrap();
        conn
    }

    // Helper: query all (name, path) rows from modules.
    fn query_modules(conn: &rusqlite::Connection) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare("SELECT name, path FROM modules ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn test_is_module_file_includes_pubspec_yaml() {
        assert!(is_module_file("pubspec.yaml"));
    }

    #[test]
    fn test_pubspec_yaml_basic_name_extraction() {
        let dir = TempDir::new().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(&pubspec, "name: my_app\n").unwrap();

        let conn = make_modules_db();
        let count = index_modules_from_files(&conn, dir.path(), &[pubspec]).unwrap();

        assert_eq!(count, 1);
        let rows = query_modules(&conn);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "my_app");
        // Root-level pubspec.yaml -> empty module path
        assert_eq!(rows[0].1, "");
    }

    #[test]
    fn test_pubspec_yaml_nested_path() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("packages").join("feature_auth");
        fs::create_dir_all(&sub).unwrap();
        let pubspec = sub.join("pubspec.yaml");
        fs::write(&pubspec, "name: feature_auth\n").unwrap();

        let conn = make_modules_db();
        let count = index_modules_from_files(&conn, dir.path(), &[pubspec]).unwrap();

        assert_eq!(count, 1);
        let rows = query_modules(&conn);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "feature_auth");
        assert_eq!(rows[0].1, "packages/feature_auth");
    }

    #[test]
    fn test_pubspec_yaml_missing_name_field_is_skipped() {
        let dir = TempDir::new().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(&pubspec, "version: 1.0.0\ndescription: No name field\n").unwrap();

        let conn = make_modules_db();
        let count = index_modules_from_files(&conn, dir.path(), &[pubspec]).unwrap();

        assert_eq!(count, 0);
        assert!(query_modules(&conn).is_empty());
    }

    #[test]
    fn test_pubspec_yaml_malformed_yaml_is_skipped() {
        let dir = TempDir::new().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(&pubspec, "name: [unclosed bracket\n").unwrap();

        let conn = make_modules_db();
        let count = index_modules_from_files(&conn, dir.path(), &[pubspec]).unwrap();

        assert_eq!(count, 0);
        assert!(query_modules(&conn).is_empty());
    }

    #[test]
    fn test_pubspec_yaml_complex_structure_extracts_only_name() {
        let dir = TempDir::new().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(
            &pubspec,
            r#"name: feature_auth
version: 2.1.0
description: Authentication feature module
environment:
  sdk: ">=3.0.0 <4.0.0"
dependencies:
  flutter:
    sdk: flutter
  http: ^1.1.0
dev_dependencies:
  flutter_test:
    sdk: flutter
"#,
        )
        .unwrap();

        let conn = make_modules_db();
        let count = index_modules_from_files(&conn, dir.path(), &[pubspec]).unwrap();

        assert_eq!(count, 1);
        let rows = query_modules(&conn);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "feature_auth");
    }

    #[test]
    fn test_pubspec_yaml_empty_name_is_skipped() {
        let dir = TempDir::new().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(&pubspec, "name: \"\"\n").unwrap();

        let conn = make_modules_db();
        let count = index_modules_from_files(&conn, dir.path(), &[pubspec]).unwrap();

        assert_eq!(count, 0);
        assert!(query_modules(&conn).is_empty());
    }

    #[test]
    fn test_pubspec_yaml_no_regression_in_gradle_parsing() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("app");
        fs::create_dir_all(&sub).unwrap();
        let gradle = sub.join("build.gradle");
        fs::write(&gradle, "android { }\n").unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(&pubspec, "name: my_app\n").unwrap();

        let conn = make_modules_db();
        let count = index_modules_from_files(&conn, dir.path(), &[gradle, pubspec]).unwrap();

        // Gradle inserts 1 (path-derived) + Flutter inserts 1
        assert_eq!(count, 2);
        let rows = query_modules(&conn);
        assert_eq!(rows.len(), 2);
        let names: Vec<&str> = rows.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"app"));
        assert!(names.contains(&"my_app"));
    }

    // Helper: create an in-memory SQLite DB with modules + module_deps tables.
    fn make_deps_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE modules (
                id   INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL,
                kind TEXT
            );
            CREATE TABLE module_deps (
                id           INTEGER PRIMARY KEY,
                module_id    INTEGER NOT NULL,
                dep_module_id INTEGER NOT NULL,
                dep_kind     TEXT,
                FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE CASCADE,
                FOREIGN KEY (dep_module_id) REFERENCES modules(id) ON DELETE CASCADE
            );",
        )
        .unwrap();
        conn
    }

    // Helper: query all (source_name, dep_name, dep_kind) rows from module_deps.
    fn query_deps(conn: &rusqlite::Connection) -> Vec<(String, String, String)> {
        let mut stmt = conn
            .prepare(
                "SELECT m1.name, m2.name, md.dep_kind
                 FROM module_deps md
                 JOIN modules m1 ON md.module_id = m1.id
                 JOIN modules m2 ON md.dep_module_id = m2.id
                 ORDER BY m1.name, md.dep_kind, m2.name",
            )
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn test_pubspec_deps_basic_dependency_extraction() {
        let dir = TempDir::new().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(
            &pubspec,
            r#"name: my_app
dependencies:
  flutter:
    sdk: flutter
  http: ^0.13.0
"#,
        )
        .unwrap();

        let mut conn = make_deps_db();
        // Pre-insert the Flutter module so index_module_dependencies can find it by path.
        conn.execute("INSERT INTO modules (name, path) VALUES ('my_app', '')", [])
            .unwrap();

        let count = index_module_dependencies(&mut conn, dir.path(), &[pubspec], false).unwrap();

        assert_eq!(count, 2);
        let deps = query_deps(&conn);
        assert_eq!(deps.len(), 2);
        let dep_names: Vec<&str> = deps.iter().map(|(_, d, _)| d.as_str()).collect();
        assert!(dep_names.contains(&"flutter"));
        assert!(dep_names.contains(&"http"));
        // All are "dependency" kind
        assert!(deps.iter().all(|(_, _, k)| k == "dependency"));
    }

    #[test]
    fn test_pubspec_deps_dev_dependencies_indexed() {
        let dir = TempDir::new().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(
            &pubspec,
            r#"name: my_app
dependencies:
  http: ^0.13.0
dev_dependencies:
  flutter_test:
    sdk: flutter
  mockito: ^5.0.0
"#,
        )
        .unwrap();

        let mut conn = make_deps_db();
        conn.execute("INSERT INTO modules (name, path) VALUES ('my_app', '')", [])
            .unwrap();

        let count = index_module_dependencies(&mut conn, dir.path(), &[pubspec], false).unwrap();

        // 1 dependency + 2 dev_dependencies
        assert_eq!(count, 3);
        let deps = query_deps(&conn);
        let dep_kinds: Vec<(&str, &str)> = deps
            .iter()
            .map(|(_, d, k)| (d.as_str(), k.as_str()))
            .collect();
        assert!(dep_kinds.contains(&("http", "dependency")));
        assert!(dep_kinds.contains(&("flutter_test", "dev_dependency")));
        assert!(dep_kinds.contains(&("mockito", "dev_dependency")));
    }

    #[test]
    fn test_pubspec_deps_missing_dependencies_section_no_rows() {
        let dir = TempDir::new().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(
            &pubspec,
            r#"name: my_app
version: 1.0.0
description: No deps here
"#,
        )
        .unwrap();

        let mut conn = make_deps_db();
        conn.execute("INSERT INTO modules (name, path) VALUES ('my_app', '')", [])
            .unwrap();

        let count = index_module_dependencies(&mut conn, dir.path(), &[pubspec], false).unwrap();

        assert_eq!(count, 0);
        assert!(query_deps(&conn).is_empty());
    }

    #[test]
    fn test_pubspec_deps_malformed_yaml_silently_skipped() {
        let dir = TempDir::new().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(
            &pubspec,
            "name: [unclosed bracket\ndependencies:\n  foo: bar\n",
        )
        .unwrap();

        let mut conn = make_deps_db();
        conn.execute("INSERT INTO modules (name, path) VALUES ('my_app', '')", [])
            .unwrap();

        let result = index_module_dependencies(&mut conn, dir.path(), &[pubspec], false);

        // Must not error out; no deps inserted
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        assert!(query_deps(&conn).is_empty());
    }
}
