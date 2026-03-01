//! Module indexing: Gradle, SPM, Perl, Maven, Flutter modules and dependencies.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;
use rusqlite::Connection;

use super::{configure_walk_ignores, find_arc_root, has_git_repo, is_excluded_dir, is_module_file};

pub fn index_modules(conn: &Connection, root: &Path) -> Result<usize> {
    use ignore::WalkBuilder;

    let is_git = has_git_repo(root);
    let arc_root = find_arc_root(root);
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(true)
        .git_ignore(is_git)
        .filter_entry(|entry| !is_excluded_dir(entry));
    configure_walk_ignores(&mut builder, arc_root.as_deref());
    let walker = builder.build();

    let files: Vec<PathBuf> = walker
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(is_module_file)
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    index_modules_from_files(conn, root, &files)
}

pub fn index_modules_from_files(
    conn: &Connection,
    root: &Path,
    files: &[PathBuf],
) -> Result<usize> {
    let mut count = 0;

    // Regex to extract SPM targets from Package.swift
    static SPM_TARGET_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"\.(?:target|testTarget|binaryTarget)\s*\(\s*name:\s*["']([^"']+)["']"#)
            .unwrap()
    });

    let spm_target_re = &*SPM_TARGET_RE;

    for path in files {
        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();

            // Android/Gradle modules
            if (name_str == "build.gradle" || name_str == "build.gradle.kts")
                && let Some(parent) = path.parent()
            {
                let module_path = parent
                    .strip_prefix(root)
                    .unwrap_or(parent)
                    .to_string_lossy()
                    .to_string();

                // Convert path to module name (e.g., features/payments/api -> features.payments.api)
                let module_name = module_path.replace('/', ".");

                if !module_name.is_empty() {
                    conn.execute(
                        "INSERT OR IGNORE INTO modules (name, path) VALUES (?1, ?2)",
                        rusqlite::params![module_name, module_path],
                    )?;
                    count += 1;
                }
            }

            // iOS/SPM modules (Package.swift)
            if name_str == "Package.swift"
                && let Some(parent) = path.parent()
            {
                let package_path = parent
                    .strip_prefix(root)
                    .unwrap_or(parent)
                    .to_string_lossy()
                    .to_string();

                // Read Package.swift and extract targets
                if let Ok(content) = fs::read_to_string(path) {
                    for caps in spm_target_re.captures_iter(&content) {
                        let target_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        if !target_name.is_empty() {
                            let module_name = if package_path.is_empty() {
                                target_name.to_string()
                            } else {
                                format!("{}.{}", package_path.replace('/', "."), target_name)
                            };
                            let module_path = if package_path.is_empty() {
                                target_name.to_string()
                            } else {
                                format!("{}/{}", package_path, target_name)
                            };

                            conn.execute(
                                "INSERT OR IGNORE INTO modules (name, path) VALUES (?1, ?2)",
                                rusqlite::params![module_name, module_path],
                            )?;
                            count += 1;
                        }
                    }
                }
            }

            // Perl modules (.pm files with package declarations)
            if name_str.ends_with(".pm")
                && let Ok(content) = fs::read_to_string(path)
            {
                static PERL_PACKAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
                    Regex::new(r"^\s*package\s+([A-Za-z_][A-Za-z0-9_:]*)\s*;").unwrap()
                });
                let re = &*PERL_PACKAGE_RE;
                {
                    for caps in re.captures_iter(&content) {
                        let package_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        if !package_name.is_empty() {
                            let module_path = path
                                .strip_prefix(root)
                                .unwrap_or(path)
                                .to_string_lossy()
                                .to_string();

                            conn.execute(
                                "INSERT OR IGNORE INTO modules (name, path) VALUES (?1, ?2)",
                                rusqlite::params![package_name, module_path],
                            )?;
                            count += 1;
                        }
                    }
                }
            }

            // Maven modules (pom.xml)
            if name_str == "pom.xml"
                && let Some(parent) = path.parent()
            {
                let module_path = parent
                    .strip_prefix(root)
                    .unwrap_or(parent)
                    .to_string_lossy()
                    .to_string();

                if let Ok(content) = fs::read_to_string(path) {
                    static ARTIFACT_RE: LazyLock<Regex> = LazyLock::new(|| {
                        Regex::new(r"<artifactId>\s*([^<]+?)\s*</artifactId>").unwrap()
                    });
                    let artifact_re = &*ARTIFACT_RE;
                    if let Some(caps) = artifact_re.captures(&content) {
                        let artifact_id = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        if !artifact_id.is_empty() {
                            let module_name = if module_path.is_empty() {
                                artifact_id.to_string()
                            } else {
                                module_path.replace('/', ".")
                            };
                            conn.execute(
                                "INSERT OR IGNORE INTO modules (name, path) VALUES (?1, ?2)",
                                rusqlite::params![module_name, module_path],
                            )?;
                            count += 1;
                        }
                    }
                }
            }

            // Flutter modules (pubspec.yaml)
            if name_str == "pubspec.yaml"
                && let Some(parent) = path.parent()
            {
                let module_path = parent
                    .strip_prefix(root)
                    .unwrap_or(parent)
                    .to_string_lossy()
                    .to_string();

                if let Ok(content) = fs::read_to_string(path) {
                    #[derive(serde::Deserialize)]
                    struct PubSpec {
                        name: Option<String>,
                    }
                    if let Ok(pubspec) = serde_yaml_ng::from_str::<PubSpec>(&content)
                        && let Some(ref mod_name) = pubspec.name
                        && !mod_name.is_empty()
                    {
                        conn.execute(
                            "INSERT OR IGNORE INTO modules (name, path) VALUES (?1, ?2)",
                            rusqlite::params![mod_name, module_path],
                        )?;
                        count += 1;
                    }
                }
            }
        }
    }

    Ok(count)
}

pub fn collect_build_files_from_db(conn: &Connection, root: &Path) -> Result<Vec<PathBuf>> {
    let mut stmt = conn.prepare("SELECT path FROM modules")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut files = Vec::new();
    for row in rows {
        let module_path = row?;
        let dir = root.join(&module_path);
        for name in &["build.gradle.kts", "build.gradle", "pom.xml"] {
            let p = dir.join(name);
            if p.exists() {
                files.push(p);
                break;
            }
        }
    }
    Ok(files)
}

pub fn index_module_dependencies(
    conn: &mut Connection,
    root: &Path,
    gradle_files: &[PathBuf],
    progress: bool,
) -> Result<usize> {
    // Regex patterns for dependency declarations
    // Gradle projects DSL style: modules { api(projects.features.payments.api) }
    static PROJECTS_DEP_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(api|implementation|compileOnly|testImplementation)\s*\(\s*projects\.([a-zA-Z_][a-zA-Z0-9_.]*)\s*\)").unwrap()
    });

    let projects_dep_re = &*PROJECTS_DEP_RE;

    // Standard Gradle style: implementation(project(":features:payments:api"))
    static GRADLE_PROJECT_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?m)(api|implementation|compileOnly|testImplementation)\s*\(\s*project\s*\(\s*["']:([^"']+)["']\s*\)"#).unwrap()
    });

    let gradle_project_re = &*GRADLE_PROJECT_RE;

    // First, ensure all modules are indexed and get their IDs
    let (module_ids, module_ids_by_path): (
        std::collections::HashMap<String, i64>,
        std::collections::HashMap<String, i64>,
    ) = {
        let mut stmt = conn.prepare("SELECT id, name, path FROM modules")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut by_name = std::collections::HashMap::new();
        let mut by_path = std::collections::HashMap::new();
        for row in rows {
            let (id, name, path) = row?;
            by_name.insert(name, id);
            by_path.insert(path, id);
        }
        (by_name, by_path)
    };

    if progress {
        eprintln!("Found {} modules in index", module_ids.len());
    }

    let mut dep_count = 0;
    let tx = conn.transaction()?;

    // Clear existing dependencies
    tx.execute("DELETE FROM module_deps", [])?;

    {
        let mut dep_stmt = tx.prepare_cached(
            "INSERT OR IGNORE INTO module_deps (module_id, dep_module_id, dep_kind) VALUES (?1, ?2, ?3)"
        )?;

        // Maven dependency regex: <dependency>...<artifactId>name</artifactId>...</dependency>
        static MAVEN_DEP_RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(
                r"(?s)<dependency>.*?<artifactId>\s*([^<]+?)\s*</artifactId>.*?</dependency>",
            )
            .unwrap()
        });
        let maven_dep_re = &*MAVEN_DEP_RE;

        for path in gradle_files {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if let Some(parent) = path.parent() {
                let module_path = parent
                    .strip_prefix(root)
                    .unwrap_or(parent)
                    .to_string_lossy()
                    .to_string();

                // Flutter dependencies (pubspec.yaml)
                if file_name == "pubspec.yaml" {
                    if let Some(&module_id) = module_ids_by_path.get(&module_path)
                        && let Ok(content) = fs::read_to_string(path)
                    {
                        #[derive(serde::Deserialize)]
                        struct PubSpecDeps {
                            dependencies: Option<serde_yaml_ng::Mapping>,
                            dev_dependencies: Option<serde_yaml_ng::Mapping>,
                        }
                        if let Ok(pubspec) = serde_yaml_ng::from_str::<PubSpecDeps>(&content) {
                            let dep_sections = [
                                (pubspec.dependencies, "dependency"),
                                (pubspec.dev_dependencies, "dev_dependency"),
                            ];
                            for (section, dep_kind) in dep_sections {
                                if let Some(deps) = section {
                                    for (key, _) in &deps {
                                        if let Some(pkg_name) = key.as_str() {
                                            if pkg_name.is_empty() {
                                                continue;
                                            }
                                            // Insert the dep package as a module if not present
                                            tx.execute(
                                                "INSERT OR IGNORE INTO modules (name, path) VALUES (?1, ?2)",
                                                rusqlite::params![pkg_name, ""],
                                            )?;
                                            let dep_id: i64 = tx.query_row(
                                                "SELECT id FROM modules WHERE name = ?1",
                                                rusqlite::params![pkg_name],
                                                |row| row.get(0),
                                            )?;
                                            dep_stmt.execute(rusqlite::params![
                                                module_id, dep_id, dep_kind
                                            ])?;
                                            dep_count += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    let module_name = module_path.replace('/', ".");

                    if let Some(&module_id) = module_ids.get(&module_name) {
                        // Read build file content
                        if let Ok(content) = fs::read_to_string(path) {
                            if file_name == "pom.xml" {
                                // Maven dependencies
                                for caps in maven_dep_re.captures_iter(&content) {
                                    let artifact_id = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                                    // Check if this artifactId matches a known module
                                    for (mod_name, &mod_id) in &module_ids {
                                        // Match by last segment (artifactId typically matches the module name)
                                        let last_segment =
                                            mod_name.rsplit('.').next().unwrap_or(mod_name);
                                        if last_segment == artifact_id {
                                            dep_stmt.execute(rusqlite::params![
                                                module_id, mod_id, "compile"
                                            ])?;
                                            dep_count += 1;
                                        }
                                    }
                                }
                            } else {
                                // Gradle dependencies
                                // Parse projects DSL style dependencies
                                for caps in projects_dep_re.captures_iter(&content) {
                                    let dep_kind =
                                        caps.get(1).map(|m| m.as_str()).unwrap_or("implementation");
                                    let dep_name = caps.get(2).map(|m| m.as_str()).unwrap_or("");

                                    if let Some(&dep_id) = module_ids.get(dep_name) {
                                        dep_stmt.execute(rusqlite::params![
                                            module_id, dep_id, dep_kind
                                        ])?;
                                        dep_count += 1;
                                    }
                                }

                                // Parse standard Gradle style dependencies
                                for caps in gradle_project_re.captures_iter(&content) {
                                    let dep_kind =
                                        caps.get(1).map(|m| m.as_str()).unwrap_or("implementation");
                                    let dep_path = caps.get(2).map(|m| m.as_str()).unwrap_or("");

                                    // Convert :features:payments:api to features.payments.api
                                    let dep_name =
                                        dep_path.trim_start_matches(':').replace(':', ".");

                                    if let Some(&dep_id) = module_ids.get(&dep_name) {
                                        dep_stmt.execute(rusqlite::params![
                                            module_id, dep_id, dep_kind
                                        ])?;
                                        dep_count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    tx.commit()?;

    Ok(dep_count)
}

/// Get dependencies of a module
pub fn get_module_deps(
    conn: &Connection,
    module_name: &str,
) -> Result<Vec<(String, String, String)>> {
    // Returns (dep_module_name, dep_module_path, dep_kind)
    let mut stmt = conn.prepare(
        r#"
        SELECT m2.name, m2.path, md.dep_kind
        FROM module_deps md
        JOIN modules m1 ON md.module_id = m1.id
        JOIN modules m2 ON md.dep_module_id = m2.id
        WHERE m1.name = ?1 OR m1.path = ?1
        ORDER BY md.dep_kind, m2.name
        "#,
    )?;

    let results = stmt
        .query_map(rusqlite::params![module_name], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

/// Get modules that depend on this module
pub fn get_module_dependents(
    conn: &Connection,
    module_name: &str,
) -> Result<Vec<(String, String, String)>> {
    // Returns (dependent_module_name, dependent_module_path, dep_kind)
    let mut stmt = conn.prepare(
        r#"
        SELECT m1.name, m1.path, md.dep_kind
        FROM module_deps md
        JOIN modules m1 ON md.module_id = m1.id
        JOIN modules m2 ON md.dep_module_id = m2.id
        WHERE m2.name = ?1 OR m2.path = ?1
        ORDER BY md.dep_kind, m1.name
        "#,
    )?;

    let results = stmt
        .query_map(rusqlite::params![module_name], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}
