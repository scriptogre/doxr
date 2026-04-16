mod config;
mod diagnostic;
mod discover;
mod extract;
mod fast_scan;
mod graph;
mod inventory;
mod parse;
mod patterns;
mod util;

use anyhow::Result;
use clap::Parser as ClapParser;
use rayon::prelude::*;
use std::path::PathBuf;
use std::process;

#[derive(ClapParser)]
#[command(
    name = "drefs",
    version,
    about = "An extremely fast Python docstring cross-reference checker, written in Rust."
)]
struct Cli {
    /// Project root directory to check (default: current directory).
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Override source directories (can be specified multiple times).
    #[arg(long = "src", short = 's')]
    src: Vec<PathBuf>,

    /// Documentation style: mkdocs, sphinx, or auto.
    #[arg(long, default_value = "auto")]
    style: String,

    /// External modules to skip (can be specified multiple times).
    #[arg(long = "known-module", short = 'k')]
    known_modules: Vec<String>,

    /// Inventory files (objects.inv) to load — URLs or local paths.
    #[arg(long = "inventory", short = 'i')]
    inventories: Vec<String>,

    /// Color output: auto, always, never.
    #[arg(long, default_value = "auto")]
    color: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Configure color output.
    match cli.color.as_str() {
        "always" => colored::control::set_override(true),
        "never" => colored::control::set_override(false),
        _ => {} // "auto" — colored crate detects TTY automatically
    }

    let project_root = cli.path.canonicalize().unwrap_or(cli.path.clone());

    // Load config from pyproject.toml, then apply CLI overrides.
    let mut config = config::DrefsConfig::load(&project_root)?;

    if !cli.src.is_empty() {
        config.src = cli.src;
    }
    if cli.style != "auto" {
        config.style = match cli.style.as_str() {
            "mkdocs" => config::DocStyle::Mkdocs,
            "sphinx" => config::DocStyle::Sphinx,
            _ => config::DocStyle::Auto,
        };
    }
    if !cli.known_modules.is_empty() {
        config.known_modules = cli.known_modules;
    }
    if !cli.inventories.is_empty() {
        config.inventories = cli.inventories;
    }

    // Resolve auto-detected style.
    config.style = config.effective_style(&project_root);

    // 1. Discover Python files.
    let src_dirs = config.effective_src(&project_root);
    let discovered = discover::discover_modules(&src_dirs, &config.exclude);

    if discovered.is_empty() {
        eprintln!("No Python files found.");
        return Ok(());
    }

    // 2. Read all files in parallel, then parse.
    //    Files without docstrings (no `"""`) and without multi-line imports
    //    use fast line-based scanning instead of tree-sitter.
    let parsed: Vec<_> = discovered
        .par_iter()
        .filter_map(|dm| {
            let content = std::fs::read(&dm.file_path).ok()?;
            let file_str = dm.file_path.display().to_string();

            let needs_tree_sitter =
                fast_scan::has_docstrings(&content) || fast_scan::has_multiline_imports(&content);

            if needs_tree_sitter {
                match parse::parse_bytes(&content, &dm.file_path, &dm.dotted_path) {
                    Ok(module) => Some((file_str, module)),
                    Err(e) => {
                        eprintln!("Warning: {}: {e}", dm.file_path.display());
                        None
                    }
                }
            } else {
                // Fast scan: line-based import/definition extraction only.
                Some((
                    file_str,
                    fast_scan::fast_scan(&content, &dm.file_path, &dm.dotted_path),
                ))
            }
        })
        .collect();

    let mut symbol_graph = graph::SymbolGraph::new();
    let mut file_map: Vec<(String, String)> = Vec::new();

    for (file_path, module) in parsed {
        file_map.push((module.path.clone(), file_path));
        symbol_graph.add_module(module);
    }

    // Expand `from X import *` into concrete imports.
    symbol_graph.expand_wildcards();
    symbol_graph.compute_roots();

    // 3. Load external inventories.
    let mut inv = inventory::Inventory::new();
    for source in &config.inventories {
        if source.starts_with("http://") || source.starts_with("https://") {
            eprintln!("Loading inventory: {source}");
            if let Err(e) = inv.load_url(source) {
                eprintln!("Warning: failed to load inventory {source}: {e}");
            }
        } else {
            let path = if std::path::Path::new(source).is_absolute() {
                PathBuf::from(source)
            } else {
                project_root.join(source)
            };
            if let Err(e) = inv.load_file(&path) {
                eprintln!("Warning: failed to load inventory {}: {e}", path.display());
            }
        }
    }

    if !inv.projects.is_empty() {
        eprintln!(
            "Loaded {} external symbols from: {}",
            inv.symbols.len(),
            inv.projects.join(", ")
        );
    }

    // 4. Check all docstrings against the graph + inventories.
    let diagnostics = diagnostic::check(&symbol_graph, &config, &inv, &file_map);

    // 5. Print results.
    for d in &diagnostics {
        println!("{}", diagnostic::format_diagnostic(d, &project_root));
    }

    if !diagnostics.is_empty() {
        eprintln!();
    }
    eprintln!("{}", diagnostic::summary(&diagnostics));

    if !diagnostics.is_empty() {
        process::exit(1);
    }

    Ok(())
}
