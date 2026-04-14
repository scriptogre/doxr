/// LSP server for doxr — provides go-to-definition and diagnostics for
/// docstring cross-references.
///
/// Pattern: lsp-server + lsp-types on stdio (same as ruff_server).
use anyhow::Result;
use lsp_server::{Connection, Message, Notification as LspNotification, Request, Response};
use lsp_types::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::DoxrConfig;
use crate::discover;
use crate::extract::{extract_references, Reference};
use crate::graph::{SourceLocation, SymbolGraph};
use crate::inventory::Inventory;
use crate::parse;

/// Convert a file path to a URI string (`file:///...`).
fn path_to_uri(path: &str) -> Uri {
    let url_str = if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    };
    url_str.parse().unwrap()
}

/// Convert a URI string to a file path.
fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let s = uri.as_str();
    let path = s.strip_prefix("file://")?;
    Some(PathBuf::from(path))
}

/// Run the LSP server on stdio.
pub fn run() -> Result<()> {
    // Ensure Ctrl+C kills the process cleanly.
    ctrlc::set_handler(|| std::process::exit(0))?;

    let (connection, io_threads) = Connection::stdio();

    let server_capabilities = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                save: Some(SaveOptions::default().into()),
                ..Default::default()
            },
        )),
        definition_provider: Some(OneOf::Left(true)),
        ..Default::default()
    })?;

    let init_params = connection.initialize(server_capabilities)?;
    let init_params: InitializeParams = serde_json::from_value(init_params)?;

    let mut server = LspState::new(init_params)?;
    server.main_loop(&connection)?;

    io_threads.join()?;
    Ok(())
}

/// Mutable server state.
struct LspState {
    graph: SymbolGraph,
    config: DoxrConfig,
    inventory: Inventory,
    uri_to_module: HashMap<String, String>,
    src_dirs: Vec<PathBuf>,
    project_root: PathBuf,
}

impl LspState {
    fn new(params: InitializeParams) -> Result<Self> {
        let project_root = params
            .root_uri
            .as_ref()
            .and_then(|uri| uri_to_path(uri))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let mut config = DoxrConfig::load(&project_root)?;
        config.style = config.effective_style(&project_root);
        let src_dirs = config.effective_src(&project_root);

        let mut state = Self {
            graph: SymbolGraph::new(),
            config,
            inventory: Inventory::new(),
            uri_to_module: HashMap::new(),
            src_dirs,
            project_root,
        };

        state.rebuild_graph();
        state.load_inventories();
        Ok(state)
    }

    fn rebuild_graph(&mut self) {
        self.graph = SymbolGraph::new();
        self.uri_to_module.clear();

        let discovered = discover::discover_modules(&self.src_dirs, &self.config.exclude);
        for dm in &discovered {
            match parse::parse_file(&dm.file_path, &dm.dotted_path) {
                Ok(module) => {
                    let uri = path_to_uri(&dm.file_path.display().to_string());
                    self.uri_to_module
                        .insert(uri.as_str().to_string(), module.path.clone());
                    self.graph.add_module(module);
                }
                Err(e) => {
                    eprintln!("doxr-lsp: warning: {}: {e}", dm.file_path.display());
                }
            }
        }
    }

    fn load_inventories(&mut self) {
        for source in &self.config.inventories {
            if source.starts_with("http://") || source.starts_with("https://") {
                let _ = self.inventory.load_url(source);
            } else {
                let path = if Path::new(source).is_absolute() {
                    PathBuf::from(source)
                } else {
                    self.project_root.join(source)
                };
                let _ = self.inventory.load_file(&path);
            }
        }
    }

    fn main_loop(&mut self, connection: &Connection) -> Result<()> {
        for msg in &connection.receiver {
            match msg {
                Message::Request(req) => {
                    if connection.handle_shutdown(&req)? {
                        return Ok(());
                    }
                    self.handle_request(req, connection)?;
                }
                Message::Notification(notif) => {
                    self.handle_notification(notif, connection)?;
                }
                Message::Response(_) => {}
            }
        }
        Ok(())
    }

    fn handle_request(&self, req: Request, connection: &Connection) -> Result<()> {
        if req.method == "textDocument/definition" {
            let params: GotoDefinitionParams = serde_json::from_value(req.params)?;
            let result = self.goto_definition(&params);
            let resp = Response::new_ok(req.id, result);
            connection.sender.send(Message::Response(resp))?;
        }
        Ok(())
    }

    fn handle_notification(
        &mut self,
        notif: LspNotification,
        connection: &Connection,
    ) -> Result<()> {
        match notif.method.as_str() {
            "textDocument/didOpen" => {
                let params: DidOpenTextDocumentParams = serde_json::from_value(notif.params)?;
                self.publish_diagnostics(connection, &params.text_document.uri)?;
            }
            "textDocument/didSave" => {
                let params: DidSaveTextDocumentParams = serde_json::from_value(notif.params)?;
                self.reparse_file(&params.text_document.uri);
                self.publish_diagnostics(connection, &params.text_document.uri)?;
            }
            "textDocument/didChange" => {
                let params: DidChangeTextDocumentParams = serde_json::from_value(notif.params)?;
                self.publish_diagnostics(connection, &params.text_document.uri)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn reparse_file(&mut self, uri: &Uri) {
        let file_path = match uri_to_path(uri) {
            Some(p) => p,
            None => return,
        };
        let dotted = match self.uri_to_module.get(uri.as_str()) {
            Some(d) => d.clone(),
            None => return,
        };
        if let Ok(module) = parse::parse_file(&file_path, &dotted) {
            self.graph.add_module(module);
        }
    }

    fn publish_diagnostics(&self, connection: &Connection, uri: &Uri) -> Result<()> {
        let module_path = match self.uri_to_module.get(uri.as_str()) {
            Some(p) => p,
            None => return Ok(()),
        };
        let module = match self.graph.modules.get(module_path) {
            Some(m) => m,
            None => return Ok(()),
        };

        let mut diagnostics = Vec::new();
        for docstring in &module.docstrings {
            let refs = extract_references(&docstring.content, &self.config.style);
            for r in refs {
                if !self.is_internal_ref(&r) {
                    continue;
                }
                if !self.graph.resolve(&r.target) && !self.inventory.contains(&r.target) {
                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position {
                                line: (docstring.line - 1) as u32,
                                character: (docstring.col - 1) as u32,
                            },
                            end: Position {
                                line: (docstring.line - 1) as u32,
                                character: (docstring.col - 1) as u32,
                            },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: Some(NumberOrString::String("DXR001".to_string())),
                        source: Some("doxr".to_string()),
                        message: format!("Unresolved reference `{}`", r.target),
                        ..Default::default()
                    });
                }
            }
        }

        let notif = LspNotification::new(
            "textDocument/publishDiagnostics".to_string(),
            PublishDiagnosticsParams {
                uri: uri.clone(),
                diagnostics,
                version: None,
            },
        );
        connection.sender.send(Message::Notification(notif))?;
        Ok(())
    }

    fn is_internal_ref(&self, reference: &Reference) -> bool {
        let root = reference.target.split('.').next().unwrap_or("");
        self.graph
            .modules
            .keys()
            .any(|path| path == root || path.starts_with(&format!("{root}.")))
    }

    fn goto_definition(&self, params: &GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = &params.text_document_position_params.position;

        let module_path = self.uri_to_module.get(uri.as_str())?;
        let module = self.graph.modules.get(module_path)?;

        // Find which docstring the cursor is in.
        let docstring = module.docstrings.iter().find(|ds| {
            let ds_line = (ds.line - 1) as u32;
            pos.line >= ds_line && pos.line <= ds_line + ds.content.lines().count() as u32
        })?;

        // Find the reference at cursor.
        let refs = extract_references(&docstring.content, &self.config.style);
        let ref_at_cursor = find_ref_at_cursor(docstring, &refs, pos)?;

        // Resolve to source location.
        let location = self.resolve_to_location(&ref_at_cursor.target)?;
        let target_uri = path_to_uri(&location.file);

        Some(GotoDefinitionResponse::Scalar(Location {
            uri: target_uri,
            range: Range {
                start: Position {
                    line: (location.line - 1) as u32,
                    character: (location.col - 1) as u32,
                },
                end: Position {
                    line: (location.line - 1) as u32,
                    character: (location.col - 1) as u32,
                },
            },
        }))
    }

    fn resolve_to_location(&self, reference: &str) -> Option<SourceLocation> {
        let segments: Vec<&str> = reference.split('.').collect();

        for split in (1..=segments.len()).rev() {
            let module_path = segments[..split].join(".");
            if let Some(module) = self.graph.modules.get(&module_path) {
                if split == segments.len() {
                    return Some(SourceLocation {
                        file: module.file_path.clone(),
                        line: 1,
                        col: 1,
                    });
                }

                let remaining = &segments[split..];
                if let Some(loc) = find_in_definitions(&module.definitions, remaining) {
                    return Some(loc);
                }

                for imp in &module.imports {
                    let local_name = imp.alias.as_deref().unwrap_or(&imp.name);
                    if local_name == remaining[0] {
                        let mut target = format!("{}.{}", imp.source, imp.name);
                        for seg in &remaining[1..] {
                            target.push('.');
                            target.push_str(seg);
                        }
                        return self.resolve_to_location(&target);
                    }
                }
            }
        }
        None
    }
}

fn find_in_definitions(
    defs: &HashMap<String, crate::graph::Symbol>,
    segments: &[&str],
) -> Option<SourceLocation> {
    let sym = defs.get(segments[0])?;
    if segments.len() == 1 {
        return sym.location.clone();
    }
    find_in_definitions(&sym.members, &segments[1..])
}

fn find_ref_at_cursor(
    docstring: &crate::graph::Docstring,
    refs: &[Reference],
    pos: &Position,
) -> Option<Reference> {
    let cursor_line = pos.line as usize;
    let ds_start_line = docstring.line - 1;

    let lines: Vec<&str> = docstring.content.lines().collect();
    let relative_line = cursor_line.checked_sub(ds_start_line)?;
    if relative_line >= lines.len() {
        return None;
    }

    let cursor_col = pos.character as usize;
    let mut cursor_offset = 0;
    for (i, line) in lines.iter().enumerate() {
        if i == relative_line {
            cursor_offset += cursor_col.min(line.len());
            break;
        }
        cursor_offset += line.len() + 1;
    }

    refs.iter()
        .find(|r| {
            let start = r.offset;
            let end = start + r.target.len();
            cursor_offset >= start && cursor_offset <= end
        })
        .cloned()
}
