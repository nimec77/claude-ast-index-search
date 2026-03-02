use anyhow::Result;
use clap::Parser;

mod cli;
use cli::{Cli, Commands, find_project_root};

use ast_index::{commands, db};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = find_project_root()?;
    let format = cli.format.as_str();

    // Migrate project DB from old kotlin-index to ast-index
    db::migrate_legacy_project(&root);

    // Compute directory scope: if cwd is inside project root, limit search to cwd subtree
    let cwd = std::env::current_dir().unwrap_or_default();
    let dir_prefix = if cwd != root {
        cwd.strip_prefix(&root).ok().map(|rel| {
            let mut s = rel.to_string_lossy().to_string();
            if !s.ends_with('/') {
                s.push('/');
            }
            s
        })
    } else {
        None
    };
    let dir_prefix_ref = dir_prefix.as_deref();

    match cli.command {
        // Grep commands
        Commands::Todo { pattern, limit } => commands::grep::cmd_todo(&root, &pattern, limit),
        Commands::Callers {
            function_name,
            limit,
        } => commands::grep::cmd_callers(&root, &function_name, limit),
        Commands::CallTree {
            function_name,
            depth,
            limit,
        } => commands::grep::cmd_call_tree(&root, &function_name, depth, limit),
        Commands::Provides { type_name, limit } => {
            commands::grep::cmd_provides(&root, &type_name, limit)
        }
        Commands::Suspend { query, limit } => {
            commands::grep::cmd_suspend(&root, query.as_deref(), limit)
        }
        Commands::Composables { query, limit } => {
            commands::grep::cmd_composables(&root, query.as_deref(), limit)
        }
        Commands::Deprecated { query, limit } => {
            commands::grep::cmd_deprecated(&root, query.as_deref(), limit)
        }
        Commands::Suppress { query, limit } => {
            commands::grep::cmd_suppress(&root, query.as_deref(), limit)
        }
        Commands::Inject { type_name, limit } => {
            commands::grep::cmd_inject(&root, &type_name, limit)
        }
        Commands::Annotations { annotation, limit } => {
            commands::grep::cmd_annotations(&root, &annotation, limit)
        }
        Commands::Deeplinks { query, limit } => {
            commands::grep::cmd_deeplinks(&root, query.as_deref(), limit)
        }
        Commands::Extensions {
            receiver_type,
            limit,
        } => commands::grep::cmd_extensions(&root, &receiver_type, limit),
        Commands::Flows { query, limit } => {
            commands::grep::cmd_flows(&root, query.as_deref(), limit)
        }
        Commands::Previews { query, limit } => {
            commands::grep::cmd_previews(&root, query.as_deref(), limit)
        }
        // Management commands
        Commands::Rebuild {
            r#type,
            no_deps,
            no_ignore,
            sub_projects,
            verbose,
            threads,
        } => {
            if let Some(t) = threads {
                // SAFETY: single-threaded at this point, no other threads reading env
                unsafe { std::env::set_var("AST_INDEX_THREADS", t.to_string()) };
            }
            commands::management::cmd_rebuild(
                &root,
                &r#type,
                !no_deps,
                no_ignore,
                sub_projects,
                verbose,
            )
        }
        Commands::Update => commands::management::cmd_update(&root),
        Commands::Restore { path } => commands::management::cmd_restore(&root, &path),
        Commands::Stats => commands::management::cmd_stats(&root, format),
        // Index commands
        Commands::Search {
            query,
            limit,
            in_file,
            module,
            fuzzy,
        } => {
            let scope = db::SearchScope {
                in_file: in_file.as_deref(),
                module: module.as_deref(),
                dir_prefix: dir_prefix_ref,
            };
            commands::index::cmd_search(&root, &query, limit, format, &scope, fuzzy)
        }
        Commands::Symbol {
            name,
            r#type,
            limit,
            in_file,
            module,
            fuzzy,
        } => {
            let scope = db::SearchScope {
                in_file: in_file.as_deref(),
                module: module.as_deref(),
                dir_prefix: dir_prefix_ref,
            };
            commands::index::cmd_symbol(
                &root,
                &name,
                r#type.as_deref(),
                limit,
                format,
                &scope,
                fuzzy,
            )
        }
        Commands::Class {
            name,
            limit,
            in_file,
            module,
            fuzzy,
        } => {
            let scope = db::SearchScope {
                in_file: in_file.as_deref(),
                module: module.as_deref(),
                dir_prefix: dir_prefix_ref,
            };
            commands::index::cmd_class(&root, &name, limit, format, &scope, fuzzy)
        }
        Commands::Implementations {
            parent,
            limit,
            in_file,
            module,
        } => {
            let scope = db::SearchScope {
                in_file: in_file.as_deref(),
                module: module.as_deref(),
                dir_prefix: dir_prefix_ref,
            };
            commands::index::cmd_implementations(&root, &parent, limit, format, &scope)
        }
        Commands::Refs { symbol, limit } => {
            commands::index::cmd_refs(&root, &symbol, limit, format)
        }
        Commands::Hierarchy { name } => commands::index::cmd_hierarchy(&root, &name),
        Commands::Usages {
            symbol,
            limit,
            in_file,
            module,
        } => {
            let scope = db::SearchScope {
                in_file: in_file.as_deref(),
                module: module.as_deref(),
                dir_prefix: dir_prefix_ref,
            };
            commands::index::cmd_usages(&root, &symbol, limit, format, &scope)
        }
        // Module commands
        Commands::Module { pattern, limit } => {
            commands::modules::cmd_module(&root, &pattern, limit)
        }
        Commands::Deps { module } => commands::modules::cmd_deps(&root, &module),
        Commands::Dependents { module } => commands::modules::cmd_dependents(&root, &module),
        Commands::UnusedDeps {
            module,
            verbose,
            no_transitive,
            no_xml,
            no_resources,
            strict,
        } => {
            let check_transitive = !no_transitive && !strict;
            let check_xml = !no_xml && !strict;
            let check_resources = !no_resources && !strict;
            commands::modules::cmd_unused_deps(
                &root,
                &module,
                verbose,
                check_transitive,
                check_xml,
                check_resources,
            )
        }
        // File commands
        Commands::File {
            pattern,
            exact,
            limit,
        } => commands::files::cmd_file(&root, &pattern, exact, limit),
        Commands::Outline { file } => commands::files::cmd_outline(&root, &file),
        Commands::Imports { file } => commands::files::cmd_imports(&root, &file),
        Commands::Api { module_path, limit } => {
            commands::files::cmd_api(&root, &module_path, limit)
        }
        Commands::Changed { base } => {
            let vcs = commands::files::detect_vcs(&root);
            let default_base = if vcs == "arc" {
                "trunk"
            } else {
                commands::files::detect_git_default_branch(&root)
            };
            let base = base.as_deref().unwrap_or(default_base);
            commands::files::cmd_changed(&root, base)
        }
        // Android commands
        Commands::XmlUsages { class_name, module } => {
            commands::android::cmd_xml_usages(&root, &class_name, module.as_deref())
        }
        Commands::ResourceUsages {
            resource,
            module,
            r#type,
            unused,
        } => commands::android::cmd_resource_usages(
            &root,
            &resource,
            module.as_deref(),
            r#type.as_deref(),
            unused,
        ),
        // iOS commands
        Commands::StoryboardUsages { class_name, module } => {
            commands::ios::cmd_storyboard_usages(&root, &class_name, module.as_deref())
        }
        Commands::AssetUsages {
            asset,
            module,
            r#type,
            unused,
        } => commands::ios::cmd_asset_usages(
            &root,
            &asset,
            module.as_deref(),
            r#type.as_deref(),
            unused,
        ),
        Commands::Swiftui { query, limit } => {
            commands::ios::cmd_swiftui(&root, query.as_deref(), limit)
        }
        Commands::AsyncFuncs { query, limit } => {
            commands::ios::cmd_async_funcs(&root, query.as_deref(), limit)
        }
        Commands::Publishers { query, limit } => {
            commands::ios::cmd_publishers(&root, query.as_deref(), limit)
        }
        Commands::MainActor { query, limit } => {
            commands::ios::cmd_main_actor(&root, query.as_deref(), limit)
        }
        // Perl commands
        Commands::PerlExports { query, limit } => {
            commands::perl::cmd_perl_exports(&root, query.as_deref(), limit)
        }
        Commands::PerlSubs { query, limit } => {
            commands::perl::cmd_perl_subs(&root, query.as_deref(), limit)
        }
        Commands::PerlPod { query, limit } => {
            commands::perl::cmd_perl_pod(&root, query.as_deref(), limit)
        }
        Commands::PerlTests { query, limit } => {
            commands::perl::cmd_perl_tests(&root, query.as_deref(), limit)
        }
        Commands::PerlImports { query, limit } => {
            commands::perl::cmd_perl_imports(&root, query.as_deref(), limit)
        }
        // Project insights
        Commands::Map {
            module,
            per_dir,
            limit,
        } => commands::project_info::cmd_map(&root, module.as_deref(), per_dir, limit, format),
        Commands::Conventions => commands::project_info::cmd_conventions(&root, format),
        Commands::UnusedSymbols {
            module,
            export_only,
            limit,
        } => commands::analysis::cmd_unused_symbols(
            &root,
            module.as_deref(),
            export_only,
            limit,
            format,
        ),
        Commands::AddRoot { path, force } => {
            commands::management::cmd_add_root(&root, &path, force)
        }
        Commands::RemoveRoot { path } => commands::management::cmd_remove_root(&root, &path),
        Commands::ListRoots => commands::management::cmd_list_roots(&root),
        Commands::Watch => commands::watch::cmd_watch(&root),
        Commands::Clear => commands::management::cmd_clear(&root),
        Commands::Version => {
            println!("ast-index v{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Commands::InstallClaudePlugin => commands::management::cmd_install_claude_plugin(),
        // Programmatic access
        Commands::Agrep {
            pattern,
            lang,
            json,
        } => commands::grep::cmd_ast_grep(&root, &pattern, lang.as_deref(), json),
        Commands::Query { sql, limit } => commands::management::cmd_query(&root, &sql, limit),
        Commands::DbPath => commands::management::cmd_db_path(&root),
        Commands::Schema => commands::management::cmd_schema(&root),
    }
}
