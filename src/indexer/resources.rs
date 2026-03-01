//! Resource indexing: Android XML/resources, iOS storyboards, assets, package managers.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;
use rusqlite::Connection;

use super::ModuleLookup;

pub struct XmlUsage {
    pub file_path: String,
    pub line: usize,
    pub class_name: String,
    pub usage_type: String,
    pub element_id: Option<String>,
}

/// Index XML layouts for class usages
pub fn index_xml_usages(
    conn: &mut Connection,
    root: &Path,
    xml_layout_files: &[PathBuf],
    progress: bool,
) -> Result<usize> {
    let module_lookup = ModuleLookup::from_db(conn)?;

    // Regex for class names in XML
    // Full class name: <com.example.MyView ...>
    static FULL_CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"<([a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)*\.[A-Z][a-zA-Z0-9_]*)").unwrap()
    });

    let full_class_re = &*FULL_CLASS_RE;
    // view class="..." or fragment android:name="..."
    static CLASS_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?:class|android:name)\s*=\s*["']([a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)*\.[A-Z][a-zA-Z0-9_]*)["']"#).unwrap()
    });

    let class_attr_re = &*CLASS_ATTR_RE;
    // android:id="@+id/xxx"
    static ID_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"android:id\s*=\s*["']@\+?id/([^"']+)["']"#).unwrap());

    let id_re = &*ID_RE;

    if progress {
        eprintln!(
            "Found {} XML layout files to index...",
            xml_layout_files.len()
        );
    }

    let tx = conn.transaction()?;

    // Clear existing XML usages
    tx.execute("DELETE FROM xml_usages", [])?;

    let mut count = 0;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO xml_usages (module_id, file_path, line, class_name, usage_type, element_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        )?;

        for xml_path in xml_layout_files {
            let rel_path = xml_path
                .strip_prefix(root)
                .unwrap_or(xml_path)
                .to_string_lossy()
                .to_string();

            // Find module for this file
            let module_id = module_lookup.find(&rel_path);

            if let Ok(content) = fs::read_to_string(xml_path) {
                for (line_num, line) in content.lines().enumerate() {
                    let line_num = line_num + 1;

                    // Extract element_id if present on this line
                    let element_id = id_re
                        .captures(line)
                        .map(|c| c.get(1).unwrap().as_str().to_string());

                    // Full class name tags
                    for caps in full_class_re.captures_iter(line) {
                        let class_name = caps.get(1).unwrap().as_str();
                        stmt.execute(rusqlite::params![
                            module_id,
                            rel_path,
                            line_num as i64,
                            class_name,
                            "view_tag",
                            element_id
                        ])?;
                        count += 1;
                    }

                    // class="..." or android:name="..." attributes
                    for caps in class_attr_re.captures_iter(line) {
                        let class_name = caps.get(1).unwrap().as_str();
                        let usage_type =
                            if line.contains("<fragment") || line.contains("android:name") {
                                "fragment"
                            } else {
                                "view_class_attr"
                            };
                        stmt.execute(rusqlite::params![
                            module_id,
                            rel_path,
                            line_num as i64,
                            class_name,
                            usage_type,
                            element_id
                        ])?;
                        count += 1;
                    }
                }
            }
        }
    }

    tx.commit()?;

    Ok(count)
}

/// Resource type
#[derive(Debug, Clone, PartialEq)]

pub enum ResourceType {
    Drawable,
    String,
    Color,
    Dimen,
    Style,
    Layout,
    Id,
    Mipmap,
    Other(String),
}

impl ResourceType {
    pub fn as_str(&self) -> &str {
        match self {
            ResourceType::Drawable => "drawable",
            ResourceType::String => "string",
            ResourceType::Color => "color",
            ResourceType::Dimen => "dimen",
            ResourceType::Style => "style",
            ResourceType::Layout => "layout",
            ResourceType::Id => "id",
            ResourceType::Mipmap => "mipmap",
            ResourceType::Other(s) => s,
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "drawable" => ResourceType::Drawable,
            "string" => ResourceType::String,
            "color" => ResourceType::Color,
            "dimen" => ResourceType::Dimen,
            "style" => ResourceType::Style,
            "layout" => ResourceType::Layout,
            "id" => ResourceType::Id,
            "mipmap" => ResourceType::Mipmap,
            other => ResourceType::Other(other.to_string()),
        }
    }
}

/// Index Android resources (drawable, string, color, etc.)
pub fn index_resources(
    conn: &mut Connection,
    root: &Path,
    res_files: &[PathBuf],
    progress: bool,
) -> Result<(usize, usize)> {
    let module_lookup = ModuleLookup::from_db(conn)?;

    if progress {
        eprintln!("Found {} resource files to analyze...", res_files.len());
    }

    let tx = conn.transaction()?;

    // Clear existing resources
    tx.execute("DELETE FROM resource_usages", [])?;
    tx.execute("DELETE FROM resources", [])?;

    let mut resource_count = 0;
    let mut usage_count = 0;

    // Regex for resource references
    static R_REF_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"R\.(drawable|string|color|dimen|style|layout|id|mipmap)\.([a-zA-Z_][a-zA-Z0-9_]*)",
        )
        .unwrap()
    });

    let r_ref_re = &*R_REF_RE;
    static XML_REF_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"@(drawable|string|color|dimen|style|layout|id|mipmap)/([a-zA-Z_][a-zA-Z0-9_]*)"#,
        )
        .unwrap()
    });

    let xml_ref_re = &*XML_REF_RE;

    // Resource definitions regex for values/*.xml
    static STRING_DEF_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"<string\s+name="([^"]+)""#).unwrap());

    let string_def_re = &*STRING_DEF_RE;
    static COLOR_DEF_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"<color\s+name="([^"]+)""#).unwrap());

    let color_def_re = &*COLOR_DEF_RE;
    static DIMEN_DEF_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"<dimen\s+name="([^"]+)""#).unwrap());

    let dimen_def_re = &*DIMEN_DEF_RE;
    static STYLE_DEF_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"<style\s+name="([^"]+)""#).unwrap());

    let style_def_re = &*STYLE_DEF_RE;

    {
        let mut res_stmt = tx.prepare_cached(
            "INSERT INTO resources (module_id, type, name, file_path, line) VALUES (?1, ?2, ?3, ?4, ?5)"
        )?;

        // First pass: index resource definitions
        for res_path in res_files {
            let rel_path = res_path
                .strip_prefix(root)
                .unwrap_or(res_path)
                .to_string_lossy()
                .to_string();

            let module_id = module_lookup.find(&rel_path);

            // Drawable files
            if (rel_path.contains("/drawable") || rel_path.contains("/mipmap"))
                && let Some(name) = res_path.file_stem().and_then(|n| n.to_str())
            {
                let res_type = if rel_path.contains("/mipmap") {
                    "mipmap"
                } else {
                    "drawable"
                };
                res_stmt.execute(rusqlite::params![module_id, res_type, name, rel_path, 1])?;
                resource_count += 1;
            }

            // Layout files
            if rel_path.contains("/layout")
                && rel_path.ends_with(".xml")
                && let Some(name) = res_path.file_stem().and_then(|n| n.to_str())
            {
                res_stmt.execute(rusqlite::params![module_id, "layout", name, rel_path, 1])?;
                resource_count += 1;
            }

            // Values files (strings, colors, dimens, styles)
            if rel_path.contains("/values")
                && rel_path.ends_with(".xml")
                && let Ok(content) = fs::read_to_string(res_path)
            {
                for (line_num, line) in content.lines().enumerate() {
                    let line_num = line_num + 1;

                    if let Some(caps) = string_def_re.captures(line) {
                        let name = caps.get(1).unwrap().as_str();
                        res_stmt.execute(rusqlite::params![
                            module_id,
                            "string",
                            name,
                            rel_path,
                            line_num as i64
                        ])?;
                        resource_count += 1;
                    }
                    if let Some(caps) = color_def_re.captures(line) {
                        let name = caps.get(1).unwrap().as_str();
                        res_stmt.execute(rusqlite::params![
                            module_id,
                            "color",
                            name,
                            rel_path,
                            line_num as i64
                        ])?;
                        resource_count += 1;
                    }
                    if let Some(caps) = dimen_def_re.captures(line) {
                        let name = caps.get(1).unwrap().as_str();
                        res_stmt.execute(rusqlite::params![
                            module_id,
                            "dimen",
                            name,
                            rel_path,
                            line_num as i64
                        ])?;
                        resource_count += 1;
                    }
                    if let Some(caps) = style_def_re.captures(line) {
                        let name = caps.get(1).unwrap().as_str();
                        res_stmt.execute(rusqlite::params![
                            module_id,
                            "style",
                            name,
                            rel_path,
                            line_num as i64
                        ])?;
                        resource_count += 1;
                    }
                }
            }
        }
    }

    // Build resource ID map: type -> name -> id (two-level for allocation-free lookup)
    let resource_ids: std::collections::HashMap<String, std::collections::HashMap<String, i64>> = {
        let mut stmt = tx.prepare("SELECT id, type, name FROM resources")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut map: std::collections::HashMap<String, std::collections::HashMap<String, i64>> =
            std::collections::HashMap::new();
        for row in rows {
            let (id, res_type, name) = row?;
            map.entry(res_type).or_default().insert(name, id);
        }
        map
    };

    // Second pass: index resource usages
    {
        let mut usage_stmt = tx.prepare_cached(
            "INSERT INTO resource_usages (resource_id, usage_file, usage_line, usage_type) VALUES (?1, ?2, ?3, ?4)"
        )?;

        // Query code files from DB instead of walking filesystem again
        let code_rel_paths: Vec<String> = {
            let mut stmt = tx.prepare("SELECT path FROM files WHERE path LIKE '%.kt' OR path LIKE '%.java' OR path LIKE '%.xml'")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };

        for rel_path in &code_rel_paths {
            let file_path = root.join(rel_path);

            if let Ok(content) = fs::read_to_string(file_path) {
                let is_xml = rel_path.ends_with(".xml");

                for (line_num, line) in content.lines().enumerate() {
                    let line_num = line_num + 1;

                    // R.type.name references (Kotlin/Java)
                    if !is_xml {
                        for caps in r_ref_re.captures_iter(line) {
                            let res_type = caps.get(1).unwrap().as_str();
                            let res_name = caps.get(2).unwrap().as_str();

                            if let Some(&resource_id) =
                                resource_ids.get(res_type).and_then(|m| m.get(res_name))
                            {
                                usage_stmt.execute(rusqlite::params![
                                    resource_id,
                                    rel_path,
                                    line_num as i64,
                                    "code"
                                ])?;
                                usage_count += 1;
                            }
                        }
                    }

                    // @type/name references (XML)
                    for caps in xml_ref_re.captures_iter(line) {
                        let res_type = caps.get(1).unwrap().as_str();
                        let res_name = caps.get(2).unwrap().as_str();

                        if let Some(&resource_id) =
                            resource_ids.get(res_type).and_then(|m| m.get(res_name))
                        {
                            usage_stmt.execute(rusqlite::params![
                                resource_id,
                                rel_path,
                                line_num as i64,
                                "xml"
                            ])?;
                            usage_count += 1;
                        }
                    }
                }
            }
        }
    }

    tx.commit()?;

    Ok((resource_count, usage_count))
}

/// Build transitive dependencies cache
pub fn build_transitive_deps(conn: &mut Connection, progress: bool) -> Result<usize> {
    // Get all direct dependencies
    let direct_deps: Vec<(i64, i64, String)> = {
        let mut stmt =
            conn.prepare("SELECT module_id, dep_module_id, dep_kind FROM module_deps")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    // Get module names
    let module_names: std::collections::HashMap<i64, String> = {
        let mut stmt = conn.prepare("SELECT id, name FROM modules")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let (id, name) = row?;
            map.insert(id, name);
        }
        map
    };

    // Build adjacency list (only api dependencies create transitive access)
    let mut api_deps: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for (module_id, dep_id, dep_kind) in &direct_deps {
        if dep_kind == "api" {
            api_deps.entry(*module_id).or_default().push(*dep_id);
        }
    }

    let tx = conn.transaction()?;

    // Clear existing
    tx.execute("DELETE FROM transitive_deps", [])?;

    let mut count = 0;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO transitive_deps (module_id, dependency_id, depth, path) VALUES (?1, ?2, ?3, ?4)"
        )?;

        let unknown = "?";

        // For each module, BFS to find all transitive dependencies
        for (module_id, dep_id, _) in &direct_deps {
            let mod_name = module_names
                .get(module_id)
                .map(|s| s.as_str())
                .unwrap_or(unknown);
            let dep_name = module_names
                .get(dep_id)
                .map(|s| s.as_str())
                .unwrap_or(unknown);

            // Direct dependency
            let path = format!("{} -> {}", mod_name, dep_name);
            stmt.execute(rusqlite::params![module_id, dep_id, 1, path])?;
            count += 1;

            // BFS for transitive (only through api deps)
            let mut visited: std::collections::HashSet<i64> = std::collections::HashSet::new();
            visited.insert(*dep_id);
            let mut queue: std::collections::VecDeque<(i64, usize, String)> =
                std::collections::VecDeque::new();

            // Add api dependencies of dep_id
            if let Some(next_deps) = api_deps.get(dep_id) {
                for &next_dep in next_deps {
                    let next_name = module_names
                        .get(&next_dep)
                        .map(|s| s.as_str())
                        .unwrap_or(unknown);
                    let next_path = format!("{} -> {} -> {}", mod_name, dep_name, next_name);
                    queue.push_back((next_dep, 2, next_path));
                }
            }

            while let Some((trans_dep, depth, path)) = queue.pop_front() {
                if visited.contains(&trans_dep) || depth > 5 {
                    continue;
                }
                visited.insert(trans_dep);

                stmt.execute(rusqlite::params![module_id, trans_dep, depth as i64, path])?;
                count += 1;

                // Continue BFS
                if let Some(next_deps) = api_deps.get(&trans_dep) {
                    for &next_dep in next_deps {
                        if !visited.contains(&next_dep) {
                            let next_name = module_names
                                .get(&next_dep)
                                .map(|s| s.as_str())
                                .unwrap_or(unknown);
                            let next_path = format!("{} -> {}", path, next_name);
                            queue.push_back((next_dep, depth + 1, next_path));
                        }
                    }
                }
            }
        }
    }

    tx.commit()?;

    if progress {
        eprintln!("Built {} transitive dependency entries", count);
    }

    Ok(count)
}

/// Parsed iOS Storyboard/XIB usage
#[derive(Debug)]

pub struct StoryboardUsage {
    pub file_path: String,
    pub line: usize,
    pub class_name: String,
    pub usage_type: String, // "viewController", "view", "cell", "segue"
    pub storyboard_id: Option<String>,
}

/// Index iOS storyboard and XIB files for class usages
pub fn index_storyboard_usages(
    conn: &mut Connection,
    root: &Path,
    storyboard_files: &[PathBuf],
    progress: bool,
) -> Result<usize> {
    let module_lookup = ModuleLookup::from_db(conn)?;

    // Regex for customClass in storyboards/xibs
    // <viewController customClass="MyViewController" ...>
    static CUSTOM_CLASS_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"customClass\s*=\s*["']([A-Z][a-zA-Z0-9_]+)["']"#).unwrap());

    let custom_class_re = &*CUSTOM_CLASS_RE;
    // storyboardIdentifier="..."
    static STORYBOARD_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?:storyboardIdentifier|identifier)\s*=\s*["']([^"']+)["']"#).unwrap()
    });

    let storyboard_id_re = &*STORYBOARD_ID_RE;

    if progress {
        eprintln!(
            "Found {} storyboard/xib files to index...",
            storyboard_files.len()
        );
    }

    let tx = conn.transaction()?;

    // Clear existing storyboard usages
    tx.execute("DELETE FROM storyboard_usages", [])?;

    let mut count = 0;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT INTO storyboard_usages (module_id, file_path, line, class_name, usage_type, storyboard_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        )?;

        for sb_path in storyboard_files {
            let rel_path = sb_path
                .strip_prefix(root)
                .unwrap_or(sb_path)
                .to_string_lossy()
                .to_string();

            // Find module for this file
            let module_id = module_lookup.find(&rel_path);

            if let Ok(content) = fs::read_to_string(sb_path) {
                for (line_num, line) in content.lines().enumerate() {
                    let line_num = line_num + 1;

                    // Extract storyboard identifier if present
                    let sb_id = storyboard_id_re
                        .captures(line)
                        .map(|c| c.get(1).unwrap().as_str().to_string());

                    // Extract custom classes
                    if let Some(caps) = custom_class_re.captures(line) {
                        let class_name = caps.get(1).unwrap().as_str();

                        // Determine usage type based on element
                        let usage_type = if line.contains("<viewController")
                            || line.contains("<tableViewController")
                            || line.contains("<collectionViewController")
                            || line.contains("<navigationController")
                            || line.contains("<tabBarController")
                        {
                            "viewController"
                        } else if line.contains("<tableViewCell")
                            || line.contains("<collectionViewCell")
                        {
                            "cell"
                        } else if line.contains("<view") || line.contains("<View") {
                            "view"
                        } else {
                            "other"
                        };

                        stmt.execute(rusqlite::params![
                            module_id,
                            rel_path,
                            line_num as i64,
                            class_name,
                            usage_type,
                            sb_id
                        ])?;
                        count += 1;
                    }
                }
            }
        }
    }

    tx.commit()?;

    if progress {
        eprintln!("Indexed {} storyboard/xib class usages", count);
    }

    Ok(count)
}

/// iOS Asset type
#[derive(Debug, Clone, PartialEq)]

pub enum IosAssetType {
    ImageSet,
    ColorSet,
    AppIcon,
    LaunchImage,
    DataSet,
    Other(String),
}

impl IosAssetType {
    pub fn as_str(&self) -> &str {
        match self {
            IosAssetType::ImageSet => "imageset",
            IosAssetType::ColorSet => "colorset",
            IosAssetType::AppIcon => "appiconset",
            IosAssetType::LaunchImage => "launchimage",
            IosAssetType::DataSet => "dataset",
            IosAssetType::Other(s) => s,
        }
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "imageset" => IosAssetType::ImageSet,
            "colorset" => IosAssetType::ColorSet,
            "appiconset" => IosAssetType::AppIcon,
            "launchimage" => IosAssetType::LaunchImage,
            "dataset" => IosAssetType::DataSet,
            other => IosAssetType::Other(other.to_string()),
        }
    }
}

/// Index iOS Assets.xcassets
pub fn index_ios_assets(
    conn: &mut Connection,
    root: &Path,
    xcassets_dirs: &[PathBuf],
    progress: bool,
) -> Result<(usize, usize)> {
    use ignore::WalkBuilder;

    let module_lookup = ModuleLookup::from_db(conn)?;

    if progress {
        eprintln!("Found {} .xcassets directories...", xcassets_dirs.len());
    }

    let tx = conn.transaction()?;

    // Clear existing iOS assets
    tx.execute("DELETE FROM ios_asset_usages", [])?;
    tx.execute("DELETE FROM ios_assets", [])?;

    let mut asset_count = 0;
    let mut usage_count = 0;

    {
        let mut asset_stmt = tx.prepare_cached(
            "INSERT INTO ios_assets (module_id, type, name, file_path) VALUES (?1, ?2, ?3, ?4)",
        )?;

        // Index assets from .xcassets directories
        for xcassets_dir in xcassets_dirs {
            let rel_xcassets = xcassets_dir
                .strip_prefix(root)
                .unwrap_or(xcassets_dir)
                .to_string_lossy()
                .to_string();

            let module_id = module_lookup.find(&rel_xcassets);

            // Walk inside xcassets to find imagesets, colorsets, etc.
            let inner_walker = WalkBuilder::new(xcassets_dir).hidden(false).build();

            for entry in inner_walker.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && matches!(
                        ext,
                        "imageset" | "colorset" | "appiconset" | "launchimage" | "dataset"
                    )
                    && let Some(name) = path.file_stem().and_then(|n| n.to_str())
                {
                    let rel_path = path
                        .strip_prefix(root)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .to_string();

                    let asset_type = IosAssetType::from_extension(ext);
                    asset_stmt.execute(rusqlite::params![
                        module_id,
                        asset_type.as_str(),
                        name,
                        rel_path
                    ])?;
                    asset_count += 1;
                }
            }
        }
    }

    // Build asset ID map
    let asset_ids: std::collections::HashMap<String, i64> = {
        let mut stmt = tx.prepare("SELECT id, name FROM ios_assets")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?))
        })?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let (name, id) = row?;
            map.insert(name, id);
        }
        map
    };

    // Index asset usages in Swift code
    // UIImage(named: "assetName") or Image("assetName") or Color("colorName")
    static SWIFT_IMAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?:UIImage\s*\(\s*named:\s*["']|Image\s*\(\s*["']|\.image\s*\(\s*named:\s*["'])([^"']+)["']"#).unwrap()
    });

    let swift_image_re = &*SWIFT_IMAGE_RE;
    static SWIFT_COLOR_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?:UIColor\s*\(\s*named:\s*["']|Color\s*\(\s*["'])([^"']+)["']"#).unwrap()
    });

    let swift_color_re = &*SWIFT_COLOR_RE;

    {
        let mut usage_stmt = tx.prepare_cached(
            "INSERT INTO ios_asset_usages (asset_id, usage_file, usage_line, usage_type) VALUES (?1, ?2, ?3, ?4)"
        )?;

        // Query swift files from DB instead of walking filesystem again
        let swift_rel_paths: Vec<String> = {
            let mut stmt = tx.prepare("SELECT path FROM files WHERE path LIKE '%.swift'")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };

        for rel_path in &swift_rel_paths {
            let file_path = root.join(rel_path);

            if let Ok(content) = fs::read_to_string(file_path) {
                for (line_num, line) in content.lines().enumerate() {
                    let line_num = line_num + 1;

                    // Image references
                    for caps in swift_image_re.captures_iter(line) {
                        let asset_name = caps.get(1).unwrap().as_str();
                        if let Some(&asset_id) = asset_ids.get(asset_name) {
                            usage_stmt.execute(rusqlite::params![
                                asset_id,
                                rel_path,
                                line_num as i64,
                                "code"
                            ])?;
                            usage_count += 1;
                        }
                    }

                    // Color references
                    for caps in swift_color_re.captures_iter(line) {
                        let asset_name = caps.get(1).unwrap().as_str();
                        if let Some(&asset_id) = asset_ids.get(asset_name) {
                            usage_stmt.execute(rusqlite::params![
                                asset_id,
                                rel_path,
                                line_num as i64,
                                "code"
                            ])?;
                            usage_count += 1;
                        }
                    }
                }
            }
        }
    }

    tx.commit()?;

    if progress {
        eprintln!("Indexed {} iOS assets, {} usages", asset_count, usage_count);
    }

    Ok((asset_count, usage_count))
}

/// Index CocoaPods and Carthage dependencies
pub fn index_ios_package_managers(conn: &Connection, root: &Path, progress: bool) -> Result<usize> {
    let mut count = 0;

    // CocoaPods: Podfile
    let podfile = root.join("Podfile");
    if podfile.exists()
        && let Ok(content) = fs::read_to_string(&podfile)
    {
        // pod 'PodName', '~> 1.0'
        static POD_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r#"pod\s+['"]([^'"]+)['"]"#).unwrap());

        let pod_re = &*POD_RE;

        for caps in pod_re.captures_iter(&content) {
            let pod_name = caps.get(1).unwrap().as_str();
            conn.execute(
                "INSERT OR IGNORE INTO modules (name, path, kind) VALUES (?1, ?2, ?3)",
                rusqlite::params![format!("pod.{}", pod_name), "Pods", "cocoapods"],
            )?;
            count += 1;
        }
    }

    // Podfile.lock for exact versions
    let podfile_lock = root.join("Podfile.lock");
    if podfile_lock.exists()
        && let Ok(content) = fs::read_to_string(&podfile_lock)
    {
        // PODS:
        //   - PodName (1.0.0)
        static POD_LOCK_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r#"^\s+-\s+([A-Za-z0-9_-]+)\s+\("#).unwrap());

        let pod_lock_re = &*POD_LOCK_RE;

        for line in content.lines() {
            if let Some(caps) = pod_lock_re.captures(line) {
                let pod_name = caps.get(1).unwrap().as_str();
                conn.execute(
                    "INSERT OR IGNORE INTO modules (name, path, kind) VALUES (?1, ?2, ?3)",
                    rusqlite::params![format!("pod.{}", pod_name), "Pods", "cocoapods"],
                )?;
                count += 1;
            }
        }
    }

    // Carthage: Cartfile
    let cartfile = root.join("Cartfile");
    if cartfile.exists()
        && let Ok(content) = fs::read_to_string(&cartfile)
    {
        // github "owner/repo" ~> 1.0
        static CARTHAGE_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r#"github\s+["']([^"']+)["']"#).unwrap());

        let carthage_re = &*CARTHAGE_RE;

        for caps in carthage_re.captures_iter(&content) {
            let repo = caps.get(1).unwrap().as_str();
            let name = repo.split('/').next_back().unwrap_or(repo);
            conn.execute(
                "INSERT OR IGNORE INTO modules (name, path, kind) VALUES (?1, ?2, ?3)",
                rusqlite::params![format!("carthage.{}", name), "Carthage/Build", "carthage"],
            )?;
            count += 1;
        }
    }

    // Carthage.resolved for exact versions
    let cartfile_resolved = root.join("Cartfile.resolved");
    if cartfile_resolved.exists()
        && let Ok(content) = fs::read_to_string(&cartfile_resolved)
    {
        static CARTHAGE_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r#"github\s+["']([^"']+)["']"#).unwrap());

        let carthage_re = &*CARTHAGE_RE;

        for caps in carthage_re.captures_iter(&content) {
            let repo = caps.get(1).unwrap().as_str();
            let name = repo.split('/').next_back().unwrap_or(repo);
            conn.execute(
                "INSERT OR IGNORE INTO modules (name, path, kind) VALUES (?1, ?2, ?3)",
                rusqlite::params![format!("carthage.{}", name), "Carthage/Build", "carthage"],
            )?;
            count += 1;
        }
    }

    if progress {
        eprintln!("Indexed {} CocoaPods/Carthage dependencies", count);
    }

    Ok(count)
}
