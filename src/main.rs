mod config;
mod diagnostic;
mod discover;
mod extract;
mod fast_scan;
mod graph;
mod inventory;
mod lsp;
mod parse;
mod patterns;

use anyhow::Result;
use clap::{Parser as ClapParser, Subcommand};
use rayon::prelude::*;
use std::path::PathBuf;
use std::process;

#[derive(ClapParser)]
#[command(
    name = "doxr",
    version,
    about = "A hyper-fast Python docstring cross-reference checker"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

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
}

#[derive(Subcommand)]
enum Command {
    /// Start the LSP server (stdio transport).
    Lsp,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(Command::Lsp) = cli.command {
        return lsp::run();
    }

    let project_root = cli.path.canonicalize().unwrap_or(cli.path.clone());

    // Load config from pyproject.toml, then apply CLI overrides.
    let mut config = config::DoxrConfig::load(&project_root)?;

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
    //    Files without docstrings (no `"""`) use fast line-based scanning
    //    instead of tree-sitter, which is ~10x cheaper per file.
    let parsed: Vec<_> = discovered
        .par_iter()
        .filter_map(|dm| {
            let content = std::fs::read(&dm.file_path).ok()?;
            let file_str = dm.file_path.display().to_string();

            if fast_scan::has_docstrings(&content) {
                // Full tree-sitter parse: extracts class members, bases,
                // docstrings, self.x attributes, etc.
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

    // 5. Print results in Ruff format.
    for d in &diagnostics {
        let rel_file = diagnostic::display_path(&d.file, &project_root);
        println!(
            "{}:{}:{}: {} {}",
            rel_file, d.line, d.col, d.code, d.message
        );
    }

    eprintln!("{}", diagnostic::summary(&diagnostics));

    if !diagnostics.is_empty() {
        process::exit(1);
    }

    Ok(())
}
