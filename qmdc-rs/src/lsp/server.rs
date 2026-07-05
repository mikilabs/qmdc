use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use walkdir::WalkDir;

/// Convert a UTF-16 code unit offset (LSP `position.character`) to a byte offset in a UTF-8 string.
/// Returns `line.len()` if the offset is past the end of the line.
/// Always returns a valid char boundary.
pub fn utf16_offset_to_byte_offset(line: &str, utf16_offset: u32) -> usize {
    let mut utf16_count = 0u32;
    for (byte_idx, ch) in line.char_indices() {
        if utf16_count >= utf16_offset {
            return byte_idx;
        }
        utf16_count += ch.len_utf16() as u32;
    }
    line.len()
}

/// Convert a byte offset in a UTF-8 string to a UTF-16 code unit offset (for LSP `Position.character`).
/// The byte_offset must be a valid char boundary. If it exceeds the string length, returns the
/// total UTF-16 length of the string.
pub fn byte_offset_to_utf16_offset(line: &str, byte_offset: usize) -> u32 {
    line[..byte_offset.min(line.len())]
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum()
}

use crate::db::QmdcDatabase;
use crate::workspace::{is_ignored, load_qmdcignore};
use crate::{parse, OutputFormat, ParseOptions};

use super::commands;
use super::document::{Document, ParsedReference};
use super::workspace::{WorkspaceIndex, WorkspaceInfo};

// Custom notification for workspace updates
pub enum WorkspaceUpdated {}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceUpdatedParams {}

impl tower_lsp::lsp_types::notification::Notification for WorkspaceUpdated {
    type Params = WorkspaceUpdatedParams;
    const METHOD: &'static str = "qmdc/workspaceUpdated";
}

pub struct Backend {
    pub(crate) client: Client,
    pub(crate) documents: Arc<RwLock<HashMap<Url, Document>>>,
    /// All discovered workspaces
    pub(crate) workspaces: Arc<RwLock<WorkspaceIndex>>,
    /// SQLite database for queries
    pub(crate) db: Arc<Mutex<QmdcDatabase>>,
    /// Whether the SQLite DB needs re-sync from workspace objects
    pub(crate) db_dirty: Arc<std::sync::atomic::AtomicBool>,
    /// Timestamp of last successful sync (epoch millis)
    db_last_sync: Arc<std::sync::atomic::AtomicU64>,
    /// Workspace folders from initialize (for rescan)
    workspace_folders: Arc<RwLock<Option<Vec<WorkspaceFolder>>>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        let db = QmdcDatabase::new().expect("Failed to create SQLite database");
        Backend {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            workspaces: Arc::new(RwLock::new(WorkspaceIndex::new())),
            db: Arc::new(Mutex::new(db)),
            db_dirty: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            db_last_sync: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            workspace_folders: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if path ends with readme.qmd.md (case-insensitive)
    fn is_readme_file(path: &str) -> bool {
        path.to_lowercase().ends_with("readme.qmd.md")
    }

    /// Check if filename is readme.qmd.md (case-insensitive)
    fn is_readme_filename(name: &std::ffi::OsStr) -> bool {
        name.to_string_lossy().to_lowercase() == "readme.qmd.md"
    }

    /// Sync SQLite database with workspace objects (only if dirty or TTL expired)
    async fn sync_sqlite(&self) {
        // Skip sync if DB is up-to-date and TTL hasn't expired (5 seconds)
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last_sync = self.db_last_sync.load(std::sync::atomic::Ordering::Acquire);
        let is_dirty = self.db_dirty.load(std::sync::atomic::Ordering::Acquire);
        let ttl_expired = now_ms.saturating_sub(last_sync) > 5000;

        if !is_dirty && !ttl_expired {
            return;
        }

        let ws_index = self.workspaces.read().await;

        // Collect all objects from all workspaces into a vector
        let mut all_objects: Vec<serde_json::Value> = Vec::new();
        for ws in ws_index.by_uri.values() {
            for objs in ws.objects.values() {
                for obj in objs {
                    let mut obj = obj.clone();
                    if let Some(map) = obj.as_object_mut() {
                        if !map.contains_key("__workspace") {
                            map.insert("__workspace".to_string(), serde_json::json!(ws.id));
                        }
                    }
                    all_objects.push(obj);
                }
            }
        }
        drop(ws_index);

        // Sync to SQLite
        if let Ok(db) = self.db.lock() {
            if let Err(e) = db.sync_objects_from_vec(&all_objects) {
                eprintln!("Failed to sync SQLite: {}", e);
            } else {
                self.db_dirty
                    .store(false, std::sync::atomic::Ordering::Release);
                self.db_last_sync
                    .store(now_ms, std::sync::atomic::Ordering::Release);
            }
        }
    }

    pub(crate) fn parse_and_index(&self, content: &str, workspace_id: Option<String>) -> Document {
        let options = ParseOptions {
            random_seed: Some(666),
            format: OutputFormat::Full, // Use Full format to get __references
        };
        let objects = parse(content, options);

        // Build id -> object index map
        let mut id_to_object = HashMap::new();
        for (idx, obj) in objects.iter().enumerate() {
            if let Some(id) = obj.get("__id").and_then(|v| v.as_str()) {
                id_to_object.insert(id.to_string(), idx);
            }
        }

        // Extract all references from __references fields
        let mut references = Vec::new();
        for obj in &objects {
            if let Some(refs) = obj.get("__references").and_then(|v| v.as_array()) {
                for r in refs {
                    if let (
                        Some(target),
                        Some(ref_type),
                        Some(line),
                        Some(start_col),
                        Some(end_col),
                    ) = (
                        r.get("target").and_then(|v| v.as_str()),
                        r.get("type").and_then(|v| v.as_str()),
                        r.get("line").and_then(|v| v.as_u64()),
                        r.get("start_col").and_then(|v| v.as_u64()),
                        r.get("end_col").and_then(|v| v.as_u64()),
                    ) {
                        references.push(ParsedReference {
                            target: target.to_string(),
                            ref_type: ref_type.to_string(),
                            line: line as u32,
                            start_col: start_col as u32,
                            end_col: end_col as u32,
                        });
                    }
                }
            }
        }

        Document {
            content: content.to_string(),
            objects,
            references,
            id_to_object,
            workspace_id,
        }
    }

    /// Get document from cache, or load from disk if not cached.
    /// This handles the case when VSCode doesn't send didOpen for already-open files.
    pub(crate) async fn get_or_load_document(&self, uri: &Url) -> Option<Document> {
        // First check cache
        {
            let docs = self.documents.read().await;
            if let Some(doc) = docs.get(uri) {
                return Some(doc.clone());
            }
        }

        // Not in cache - try to load from disk
        eprintln!("[qmdc]   Document not in cache, loading from disk...");
        let path = uri.to_file_path().ok()?;
        let content = std::fs::read_to_string(&path).ok()?;

        // Find workspace ID for this file
        let ws_id = {
            let ws_index = self.workspaces.read().await;
            self.find_workspace_for_file(uri, &ws_index)
                .map(|ws| ws.id.clone())
        };

        let doc = self.parse_and_index(&content, ws_id);

        // Add to cache for future requests
        {
            let mut docs = self.documents.write().await;
            docs.insert(uri.clone(), doc.clone());
        }

        eprintln!(
            "[qmdc]   Loaded document from disk: {} objects, {} references",
            doc.objects.len(),
            doc.references.len()
        );

        Some(doc)
    }

    /// Find all workspaces (including nested) in a folder
    async fn find_all_workspaces(&self, folder_uri: &Url) -> Vec<WorkspaceInfo> {
        let mut workspaces = Vec::new();
        let folder_path = match folder_uri.to_file_path() {
            Ok(p) => p,
            Err(_) => {
                eprintln!(
                    "[LSP Debug] Failed to convert folder URI to path: {}",
                    folder_uri
                );
                return workspaces;
            }
        };

        eprintln!(
            "[LSP Debug] Searching for workspaces in: {}",
            folder_path.display()
        );

        // Load .qmdcignore patterns from project root
        let ignore_set = load_qmdcignore(&folder_path);

        // Find all directories with readme.qmd.md containing __Workspace
        let mut workspace_roots: Vec<PathBuf> = Vec::new();
        let mut readme_count = 0;
        let mut workspace_count = 0;

        for entry in WalkDir::new(&folder_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Check .qmdcignore before processing
            if is_ignored(path, &folder_path, &ignore_set) {
                continue;
            }

            if path
                .file_name()
                .map(Self::is_readme_filename)
                .unwrap_or(false)
            {
                readme_count += 1;
                if let Ok(content) = std::fs::read_to_string(path) {
                    let doc = self.parse_and_index(&content, None);
                    // Check if this readme defines a __Workspace
                    let has_workspace = doc.objects.iter().any(|obj| {
                        obj.get("__kind").and_then(|v| v.as_str()) == Some("__Workspace")
                    });
                    if has_workspace {
                        workspace_count += 1;
                        if let Some(parent) = path.parent() {
                            eprintln!("[LSP Debug] Found workspace at: {}", parent.display());
                            workspace_roots.push(parent.to_path_buf());
                        }
                    }
                }
            }
        }

        eprintln!(
            "[LSP Debug] Scanned {} readme.qmd.md files, found {} workspaces",
            readme_count, workspace_count
        );

        // Sort by path length (shorter = parent workspace)
        workspace_roots.sort_by_key(|p| p.as_os_str().len());

        // Scan each workspace, excluding files that belong to nested workspaces
        for ws_root in &workspace_roots {
            let ws_uri = Url::from_file_path(ws_root).ok();
            if let Some(uri) = ws_uri {
                // Get nested workspace roots (children of this workspace)
                let nested: Vec<&PathBuf> = workspace_roots
                    .iter()
                    .filter(|p| *p != ws_root && p.starts_with(ws_root))
                    .collect();

                eprintln!(
                    "[LSP Debug] Scanning workspace folder: {}",
                    ws_root.display()
                );
                // Pass project root for relative path calculation and ignore set
                if let Some(info) = self
                    .scan_workspace_folder(&uri, &nested, &folder_path, &ignore_set)
                    .await
                {
                    eprintln!(
                        "[LSP Debug] Successfully loaded workspace '{}' with {} files",
                        info.id,
                        info.files.len()
                    );
                    workspaces.push(info);
                } else {
                    eprintln!(
                        "[LSP Debug] Failed to load workspace at: {} (no QMD.md files found?)",
                        ws_root.display()
                    );
                }
            }
        }

        workspaces
    }

    /// Scan a folder for QMDC workspace (looks for readme.qmd.md with [[id:__Workspace]])
    async fn scan_workspace_folder(
        &self,
        folder_uri: &Url,
        _exclude: &[&PathBuf],
        project_root: &Path,
        _ignore_set: &Option<globset::GlobSet>,
    ) -> Option<WorkspaceInfo> {
        let folder_path = folder_uri.to_file_path().ok()?;

        // Find readme.qmd.md case-insensitive
        let readme_path = std::fs::read_dir(&folder_path)
            .ok()?
            .filter_map(|entry| entry.ok())
            .find(|entry| {
                entry.path().is_file() && Self::is_readme_filename(entry.file_name().as_ref())
            })
            .map(|entry| entry.path());

        let (ws_id, def_line) = if let Some(ref readme_path) = readme_path {
            if let Ok(content) = std::fs::read_to_string(readme_path) {
                let doc = self.parse_and_index(&content, None);

                // Find workspace object
                if let Some(ws_obj) = doc
                    .objects
                    .iter()
                    .find(|obj| obj.get("__kind").and_then(|v| v.as_str()) == Some("__Workspace"))
                {
                    let id = ws_obj
                        .get("__id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            folder_path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "workspace".to_string())
                        });
                    let line = ws_obj.get("__line").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                    (id, line)
                } else {
                    // readme file exists but no __Workspace - use folder name
                    (
                        folder_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "workspace".to_string()),
                        0,
                    )
                }
            } else {
                // Can't read readme file - use folder name
                (
                    folder_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "workspace".to_string()),
                    0,
                )
            }
        } else {
            // No readme file found - use folder name as workspace ID
            (
                folder_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "workspace".to_string()),
                0,
            )
        };

        // Use parse_workspace from workspace.rs to get proper __namespace and __parent
        // This ensures LSP and CLI use the same parsing logic
        use crate::workspace::parse_workspace;
        use crate::OutputFormat;

        let ws_result = parse_workspace(&folder_path, OutputFormat::Full);

        // Convert WorkspaceResult to our format
        let files: Vec<std::path::PathBuf> = ws_result
            .files
            .iter()
            .map(|f| folder_path.join(f))
            .collect();

        let mut objects: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
        let mut file_to_ids: HashMap<String, Vec<String>> = HashMap::new();

        for mut obj in ws_result.objects {
            let id_opt = obj
                .get("__id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let file_opt = obj
                .get("__file")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Adjust __file paths to be relative to project_root (not workspace root)
            // This matches the behavior of parse_all_workspaces
            if let Some(obj_map) = obj.as_object_mut() {
                if let Some(file_path) = file_opt.as_ref() {
                    let full_path = folder_path.join(file_path);
                    if let Ok(rel_path) = full_path.strip_prefix(project_root) {
                        obj_map.insert(
                            "__file".to_string(),
                            serde_json::json!(rel_path.to_string_lossy().to_string()),
                        );
                    }
                }
            }

            if let Some(id) = id_opt {
                // Build file URI for tracking using adjusted __file path
                let adjusted_file = obj.get("__file").and_then(|v| v.as_str());
                if let Some(file_path) = adjusted_file {
                    let full_path = project_root.join(file_path);
                    let file_uri = Url::from_file_path(&full_path)
                        .ok()
                        .map(|u| u.to_string())
                        .unwrap_or_default();
                    file_to_ids.entry(file_uri).or_default().push(id.clone());
                }
                objects.entry(id).or_default().push(obj);
            }
        }

        // Build by_local_id index from all objects for __local_id fallback resolution
        let mut by_local_id: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
        for objs in objects.values() {
            for obj in objs {
                if let Some(local_id) = obj.get("__local_id").and_then(|v| v.as_str()) {
                    if !local_id.is_empty() {
                        by_local_id
                            .entry(local_id.to_string())
                            .or_default()
                            .push(obj.clone());
                    }
                }
            }
        }

        // Only return if we found some QMD.md files
        if files.is_empty() {
            eprintln!(
                "[LSP Debug] Workspace '{}' at {} has no QMD.md files, skipping",
                ws_id,
                folder_path.display()
            );
            return None;
        }

        eprintln!(
            "[LSP Debug] Workspace '{}' at {} has {} QMD.md files",
            ws_id,
            folder_path.display(),
            files.len()
        );

        Some(WorkspaceInfo {
            id: ws_id,
            root_uri: folder_uri.clone(),
            root_path: folder_path,
            project_root: project_root.to_path_buf(),
            files,
            objects,
            by_local_id,
            file_to_ids,
            def_line,
        })
    }

    /// Initialize workspaces from VS Code workspace folders
    async fn init_workspaces(&self, folders: Option<Vec<WorkspaceFolder>>) {
        // Save workspace folders for rescan
        {
            let mut ws_folders = self.workspace_folders.write().await;
            *ws_folders = folders.clone();
        }

        let Some(folders) = folders else { return };

        let mut ws_index = self.workspaces.write().await;

        for folder in folders {
            // Find all workspaces (including nested ones) in this folder
            let workspaces = self.find_all_workspaces(&folder.uri).await;

            for info in workspaces {
                self.client
                    .log_message(
                        MessageType::INFO,
                        format!(
                            "Found QMDC workspace '{}' at {}",
                            info.id,
                            info.root_path.display()
                        ),
                    )
                    .await;
                ws_index.add(info);
            }
        }

        // Report duplicate workspace IDs
        for (id, workspaces) in ws_index.get_duplicates() {
            let locations: Vec<String> = workspaces
                .iter()
                .map(|w| w.root_path.display().to_string())
                .collect();

            self.client.log_message(
                MessageType::WARNING,
                format!(
                    "Duplicate workspace ID '{}' found in: {}. Cross-workspace references will not resolve.",
                    id,
                    locations.join(", ")
                )
            ).await;
        }
    }

    /// Rescan all workspaces (full rescan for structural changes)
    async fn rescan_workspaces(&self) {
        eprintln!("[LSP] Rescanning workspaces...");

        let folders = {
            let ws_folders = self.workspace_folders.read().await;
            ws_folders.clone()
        };

        let Some(folders) = folders else {
            eprintln!("[LSP] No workspace folders to rescan");
            return;
        };

        // Snapshot currently-open document buffers BEFORE clearing the cache.
        // The editor still has these files open, so after the index is rebuilt
        // we must re-parse them and re-publish their diagnostics — otherwise
        // stale diagnostics (e.g. a broken_link to an object whose file was
        // just created) linger until a server restart. See QMD-58.
        let open_buffers: Vec<(Url, String)> = {
            let docs = self.documents.read().await;
            docs.iter()
                .map(|(u, d)| (u.clone(), d.content.clone()))
                .collect()
        };

        // Clear existing workspaces and documents cache
        {
            let mut ws_index = self.workspaces.write().await;
            *ws_index = WorkspaceIndex::new();
        }
        self.db_dirty
            .store(true, std::sync::atomic::Ordering::Release);

        // Clear documents cache to avoid stale data
        {
            let mut docs = self.documents.write().await;
            docs.clear();
        }

        // Re-initialize workspaces
        self.init_workspaces(Some(folders)).await;

        // Sync SQLite after rescan
        eprintln!("[LSP] Rescan complete, syncing SQLite...");
        self.sync_sqlite().await;
        eprintln!("[LSP] SQLite synced after rescan");

        // Re-parse the previously-open buffers against the freshly-built index
        // and re-publish their diagnostics so the editor matches the CLI
        // validator without requiring a restart. See QMD-58.
        for (uri, content) in open_buffers {
            let ws_id = {
                let ws_index = self.workspaces.read().await;
                self.find_workspace_for_file(&uri, &ws_index)
                    .map(|ws| ws.id.clone())
            };
            let doc = self.parse_and_index(&content, ws_id);
            self.publish_diagnostics(uri.clone(), &doc).await;
            {
                let mut docs = self.documents.write().await;
                docs.insert(uri, doc);
            }
        }

        // Notify extension
        self.client
            .send_notification::<WorkspaceUpdated>(WorkspaceUpdatedParams {})
            .await;
        eprintln!("[LSP] Rescan notification sent");
    }

    /// Find which workspace a file belongs to
    pub(crate) fn find_workspace_for_file<'a>(
        &self,
        file_uri: &Url,
        index: &'a WorkspaceIndex,
    ) -> Option<&'a WorkspaceInfo> {
        let file_path = file_uri.to_file_path().ok()?;

        // Find workspace whose root is a prefix of this file's path
        // Note: nested workspaces are not allowed, so there should be only one match
        index
            .by_uri
            .values()
            .find(|ws| file_path.starts_with(&ws.root_path))
    }

    /// Get the UTF-16 character offset at the end of a line in a document.
    /// Returns 100 (approximate) if the document or line is not found.
    pub(crate) async fn get_line_end_character(&self, uri: &Url, line: u32) -> u32 {
        let docs = self.documents.read().await;
        docs.get(uri)
            .and_then(|doc| doc.content.lines().nth(line as usize))
            .map(|l| byte_offset_to_utf16_offset(l, l.len()))
            .unwrap_or(100)
    }

    /// Split a dot-path reference into (obj_prefix, field_part).
    /// Returns None if the target doesn't look like a field reference
    /// (no dot, contains colon, or empty parts).
    pub(crate) fn split_field_ref(raw_target: &str) -> Option<(&str, &str)> {
        // Single source of truth: delegate to the shared core resolver.
        crate::core::resolve::split_field_ref(raw_target)
    }

    /// Check if a dot-path reference resolves as a field reference (obj.field).
    /// Check if an object matches as a child of a given parent with a specific field.
    /// `__parent` stores values like `"[[#team]]"`, so we match exactly.
    fn is_child_of_parent(obj: &serde_json::Value, parent_id: &str, field_part: &str) -> bool {
        let obj_parent = obj.get("__parent").and_then(|v| v.as_str()).unwrap_or("");
        let obj_parent_field = obj
            .get("__parent_field")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let obj_id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");
        let expected_parent_ref = format!("[[#{}]]", parent_id);

        obj_parent == expected_parent_ref
            && (obj_parent_field == field_part || obj_id == field_part)
    }

    /// Resolve a field reference (e.g., "quickstart.content") to a target location.
    /// Returns (uri, line) if the field can be navigated to.
    pub(crate) async fn resolve_field_ref_location(
        &self,
        raw_target: &str,
        uri: &Url,
    ) -> Option<(Url, u32)> {
        let (obj_prefix, field_part) = Self::split_field_ref(raw_target)?;

        // Find the parent object
        let (parent_obj, parent_uri) = self
            .find_object_in_workspace_with_namespace(obj_prefix, None, uri)
            .await?;

        // Check if the field exists on this object
        if parent_obj.get(field_part).is_none() || field_part.starts_with("__") {
            return None;
        }

        let target_uri = parent_uri.unwrap_or_else(|| uri.clone());
        let parent_id = parent_obj
            .get("__id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Strategy 1: Look for a child object with matching __parent_field
        let child_line = {
            let ws_index = self.workspaces.read().await;
            if let Some(ws) = self.find_workspace_for_file(uri, &ws_index) {
                let mut found_line: Option<u32> = None;
                for objs in ws.objects.values() {
                    for obj in objs {
                        if Self::is_child_of_parent(obj, parent_id, field_part) {
                            found_line = obj
                                .get("__line")
                                .and_then(|v| v.as_u64())
                                .map(|l| l as u32 - 1);
                            break;
                        }
                    }
                    if found_line.is_some() {
                        break;
                    }
                }
                found_line
            } else {
                // Try in current document
                let docs = self.documents.read().await;
                if let Some(doc) = docs.get(&target_uri) {
                    doc.objects.iter().find_map(|obj| {
                        if Self::is_child_of_parent(obj, parent_id, field_part) {
                            obj.get("__line")
                                .and_then(|v| v.as_u64())
                                .map(|l| l as u32 - 1)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            }
        };

        if let Some(line) = child_line {
            return Some((target_uri, line));
        }

        // Strategy 2: Use __positions to find the field's heading line
        if let Some(pos_line) = parent_obj
            .get("__positions")
            .and_then(|p| p.get(field_part))
            .and_then(|pos| pos.get("line"))
            .and_then(|l| l.as_u64())
            .map(|l| l as u32 - 1)
        {
            // Find the heading line (pos_line itself or scan backwards)
            let heading_line = {
                let docs = self.documents.read().await;
                if let Some(doc) = docs.get(&target_uri) {
                    let lines: Vec<&str> = doc.content.lines().collect();
                    if let Some(line_content) = lines.get(pos_line as usize) {
                        if line_content.trim_start().starts_with('#') {
                            pos_line
                        } else {
                            // Scan backwards to find the heading
                            (0..pos_line)
                                .rev()
                                .find(|&l| {
                                    lines
                                        .get(l as usize)
                                        .is_some_and(|lc| lc.trim_start().starts_with('#'))
                                })
                                .unwrap_or(pos_line)
                        }
                    } else {
                        pos_line
                    }
                } else {
                    pos_line
                }
            };
            return Some((target_uri, heading_line));
        }

        // Strategy 3: Fall back to parent object line
        if let Some(line) = parent_obj.get("__line").and_then(|v| v.as_u64()) {
            return Some((target_uri, line as u32 - 1));
        }

        None
    }

    /// Backfill `__workspace` / `__namespace` onto an object parsed from the open buffer.
    ///
    /// The LSP's single-file parse of a live document doesn't know either value (they are
    /// assigned during workspace indexing), so derived values like the hover global id would
    /// read as the `workspace::id` placeholder instead of the real `ws:ns:id`. This fills the
    /// two fields from the indexed sibling objects of the same file (keeping them byte-identical
    /// to the index), only when they are currently empty/absent. No-op when the file is not in
    /// any indexed workspace (e.g. a standalone single-file session).
    pub(crate) async fn backfill_ws_ns(&self, obj: &mut serde_json::Value, uri: &Url) {
        let ws_empty = obj
            .get("__workspace")
            .and_then(|v| v.as_str())
            .map(|s| s.is_empty())
            .unwrap_or(true);
        let ns_empty = obj
            .get("__namespace")
            .and_then(|v| v.as_str())
            .map(|s| s.is_empty())
            .unwrap_or(true);
        if !ws_empty && !ns_empty {
            return;
        }

        let ws_index = self.workspaces.read().await;
        let ws = match self.find_workspace_for_file(uri, &ws_index) {
            Some(ws) => ws,
            None => return,
        };

        // Workspace id + the file's namespace, taken from the indexed sibling objects of this
        // same file. Fall back to the workspace's own id (namespace stays empty) when the file
        // has no indexed objects yet.
        let (ws_id, ns) = ws
            .file_to_ids
            .get(uri.as_str())
            .and_then(|ids| {
                ids.iter().find_map(|id| {
                    ws.objects.get(id).and_then(|objs| {
                        objs.iter().find_map(|o| {
                            let w = o.get("__workspace").and_then(|v| v.as_str()).unwrap_or("");
                            let n = o.get("__namespace").and_then(|v| v.as_str()).unwrap_or("");
                            if !w.is_empty() || !n.is_empty() {
                                Some((w.to_string(), n.to_string()))
                            } else {
                                None
                            }
                        })
                    })
                })
            })
            .unwrap_or_else(|| (ws.id.clone(), String::new()));

        if let Some(map) = obj.as_object_mut() {
            if ws_empty && !ws_id.is_empty() {
                map.insert("__workspace".to_string(), serde_json::json!(ws_id));
            }
            if ns_empty && !ns.is_empty() {
                map.insert("__namespace".to_string(), serde_json::json!(ns));
            }
        }
    }

    /// Find object by ID, first in current document, then in workspace
    /// If multiple objects with same ID exist, prefer object from same namespace as current file
    /// If ref_namespace is provided, prefer object from that namespace
    pub(crate) async fn find_object_in_workspace_with_namespace(
        &self,
        id: &str,
        ref_namespace: Option<&str>,
        current_uri: &Url,
    ) -> Option<(serde_json::Value, Option<Url>)> {
        // First try current document. The single-file parse of the open buffer carries no
        // __workspace / __namespace, so a same-file reference would otherwise render a
        // "workspace::id" placeholder global id in hover. Backfill both from the workspace
        // index before returning. (Lock order: take documents, drop it, then take
        // workspaces — matching the workspaces→documents order used below.)
        let doc_obj = {
            let docs = self.documents.read().await;
            docs.get(current_uri)
                .and_then(|doc| self.find_object_by_id(doc, id).cloned())
        };
        if let Some(mut obj) = doc_obj {
            self.backfill_ws_ns(&mut obj, current_uri).await;
            return Some((obj, Some(current_uri.clone())));
        }

        // Then try workspace — object matching goes through the shared core resolver
        // (single source of truth), keeping only the LSP-specific URI derivation here.
        let ws_index = self.workspaces.read().await;
        if let Some(ws) = self.find_workspace_for_file(current_uri, &ws_index) {
            // Namespace of current file (from current document's objects).
            let current_namespace = {
                let docs = self.documents.read().await;
                docs.get(current_uri).and_then(|doc| {
                    doc.objects
                        .iter()
                        .find_map(|obj| obj.get("__namespace").and_then(|v| v.as_str()))
                        .map(|ns| ns.to_string())
                })
            };
            // Priority: ref_namespace (if specified), then current namespace.
            let target_namespace = ref_namespace.map(|s| s.to_string()).or(current_namespace);

            let ws_objects: Vec<serde_json::Value> =
                ws.objects.values().flatten().cloned().collect();
            let idx = crate::core::resolve::ObjectIndex::build(&ws_objects);

            if let Some(obj) = idx.resolve_id(id, target_namespace.as_deref()) {
                // __file is relative to project_root.
                let file = obj.get("__file").and_then(|v| v.as_str()).and_then(|f| {
                    let full_path = ws.project_root.join(f);
                    Url::from_file_path(&full_path).ok()
                });
                return Some((obj.clone(), file));
            }

            eprintln!(
                "[qmdc] Object '{}' not found in workspace '{}', available: {:?}",
                id,
                ws.id,
                ws.objects.keys().take(10).collect::<Vec<_>>()
            );
        } else {
            eprintln!("[qmdc] No workspace found for file: {}", current_uri);
        }

        None
    }

    /// Find object by ID, first in current document, then in workspace
    /// If multiple objects with same ID exist, prefer object from same namespace as current file
    pub(crate) async fn find_object_in_workspace(
        &self,
        id: &str,
        current_uri: &Url,
    ) -> Option<(serde_json::Value, Option<Url>)> {
        self.find_object_in_workspace_with_namespace(id, None, current_uri)
            .await
    }

    /// Resolve cross-workspace reference like [[#ws_id:id]] or [[#ws_id:ns:id]]
    pub(crate) async fn resolve_cross_workspace_ref(
        &self,
        ws_id: &str,
        id: &str,
    ) -> Option<(serde_json::Value, Url)> {
        let ws_index = self.workspaces.read().await;

        // Check for ambiguous workspace ID
        if ws_index.is_ambiguous(ws_id) {
            return None; // Ambiguous, can't resolve
        }

        let ws = ws_index.get_by_id(ws_id)?;
        let objs = ws.objects.get(id)?;
        let obj = objs.first()?;

        // __file is now relative to project_root (not workspace root)
        let file_uri = obj.get("__file").and_then(|v| v.as_str()).and_then(|f| {
            let full_path = ws.project_root.join(f);
            Url::from_file_path(&full_path).ok()
        })?;

        Some((obj.clone(), file_uri))
    }

    async fn publish_diagnostics(&self, uri: Url, doc: &Document) {
        let diagnostics = self.compute_diagnostics(&uri, doc).await;
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    /// Collect the set of object `__id`s defined in a document.
    fn object_ids(doc: &Document) -> std::collections::HashSet<String> {
        doc.objects
            .iter()
            .filter_map(|o| {
                o.get("__id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect()
    }

    /// Re-publish diagnostics for every currently-open document against the
    /// current workspace index.
    ///
    /// This is the cross-file diagnostics refresh that makes the editor match
    /// `qmdc workspace validate` without a server restart: when a new object id
    /// appears (e.g. a referenced target file is created), documents that hold
    /// now-resolvable references get their stale `broken_link` diagnostics
    /// cleared. See QMD-58.
    ///
    /// Uses the already-cached `Document` for each open file — content is
    /// unchanged, only reference *resolution* (against `self.workspaces`) needs
    /// recomputing, so no reparse is required. Documents are snapshotted (cloned)
    /// and the lock dropped before `publish_diagnostics` runs, so no document
    /// lock is held across an await. The cache is NOT written back, so this can
    /// never clobber a concurrently-edited document.
    async fn refresh_open_documents(&self) {
        let snapshot: Vec<(Url, Document)> = {
            let docs = self.documents.read().await;
            docs.iter().map(|(u, d)| (u.clone(), d.clone())).collect()
        };

        for (uri, doc) in snapshot {
            self.publish_diagnostics(uri, &doc).await;
        }
    }

    /// Re-publish diagnostics for all open documents except `except`.
    ///
    /// Used when an edit to one document changes the set of object ids it
    /// defines: references to those ids in OTHER open documents must be
    /// re-evaluated (a newly-added anchor resolves links elsewhere; a removed
    /// anchor breaks them). The edited document is published by its own handler,
    /// so it is skipped here. Like `refresh_open_documents`, this only
    /// re-publishes diagnostics from cached docs and never writes the cache back.
    /// See QMD-58.
    async fn refresh_other_open_documents(&self, except: &Url) {
        let snapshot: Vec<(Url, Document)> = {
            let docs = self.documents.read().await;
            docs.iter()
                .filter(|(u, _)| *u != except)
                .map(|(u, d)| (u.clone(), d.clone()))
                .collect()
        };

        for (uri, doc) in snapshot {
            self.publish_diagnostics(uri, &doc).await;
        }
    }

    async fn compute_diagnostics(&self, uri: &Url, doc: &Document) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check for duplicate IDs
        let mut seen_ids: HashMap<String, (u32, String)> = HashMap::new(); // id -> (line, label)
        for obj in &doc.objects {
            if let (Some(id), Some(line)) = (
                obj.get("__id").and_then(|v| v.as_str()),
                obj.get("__line").and_then(|v| v.as_u64()),
            ) {
                let label = obj.get("__label").and_then(|v| v.as_str()).unwrap_or(id);
                let line = line as u32 - 1; // Parser uses 1-based, LSP uses 0-based

                if let Some((first_line, _)) = seen_ids.get(id) {
                    let lines: Vec<&str> = doc.content.lines().collect();
                    let line_content = lines.get(line as usize).unwrap_or(&"");

                    // Find the [[id]] span in the line for precise highlighting
                    let (start_char, end_char) = {
                        let pattern = format!("[[{}]]", id);
                        if let Some(byte_start) = line_content.find(&pattern) {
                            let byte_end = byte_start + pattern.len();
                            (
                                byte_offset_to_utf16_offset(line_content, byte_start),
                                byte_offset_to_utf16_offset(line_content, byte_end),
                            )
                        } else {
                            // Fallback: highlight full line
                            (
                                0,
                                byte_offset_to_utf16_offset(line_content, line_content.len()),
                            )
                        }
                    };

                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position {
                                line,
                                character: start_char,
                            },
                            end: Position {
                                line,
                                character: end_char,
                            },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: Some(NumberOrString::String("QMDC003".to_string())),
                        source: Some("qmdc".to_string()),
                        message: format!(
                            "Duplicate ID '{}' (first defined on line {})",
                            id,
                            first_line + 1
                        ),
                        ..Default::default()
                    });
                } else {
                    seen_ids.insert(id.to_string(), (line, label.to_string()));
                }
            }
        }

        // Check for __Workspace in wrong file (must be in readme.qmd.md)
        if let Ok(path) = uri.to_file_path() {
            if let Some(file_name) = path.file_name() {
                if !Self::is_readme_filename(file_name) {
                    // Check if this document contains a __Workspace object
                    for obj in &doc.objects {
                        if obj.get("__kind").and_then(|v| v.as_str()) == Some("__Workspace") {
                            if let Some(line) = obj.get("__line").and_then(|v| v.as_u64()) {
                                let line = line as u32 - 1; // Parser uses 1-based, LSP uses 0-based
                                let lines: Vec<&str> = doc.content.lines().collect();
                                let line_content = lines.get(line as usize).unwrap_or(&"");
                                let ws_id = obj.get("__id").and_then(|v| v.as_str()).unwrap_or("");

                                diagnostics.push(Diagnostic {
                                    range: Range {
                                        start: Position { line, character: 0 },
                                        end: Position {
                                            line,
                                            character: byte_offset_to_utf16_offset(
                                                line_content,
                                                line_content.len(),
                                            ),
                                        },
                                    },
                                    severity: Some(DiagnosticSeverity::ERROR),
                                    code: Some(NumberOrString::String("QMDC004".to_string())),
                                    source: Some("qmdc".to_string()),
                                    message: format!(
                                        "Workspace '{}' must be defined in readme.qmd.md, not here",
                                        ws_id
                                    ),
                                    ..Default::default()
                                });
                            }
                        }
                    }
                }
            }
        }

        // Broken / ambiguous links — delegate to the SINGLE shared resolver that the
        // CLI and MCP `validate` use (core::ops::validate::collect_reference_issues),
        // so the LSP and the validator can never disagree again. We diagnose the OPEN
        // doc's freshly-parsed objects (always current), resolving against the whole
        // workspace. The open doc's single-file parse has no namespace, so we backfill
        // it from the workspace index (the one source that knows it) — that backfill is
        // what makes same-namespace local-ids like `[[#res_guide]]` resolve.
        {
            let ws_index = self.workspaces.read().await;
            let ws_opt = self.find_workspace_for_file(uri, &ws_index);

            // Namespace for the open file, recovered from the indexed copy.
            let doc_ns: String = ws_opt
                .and_then(|ws| ws.file_to_ids.get(uri.as_str()))
                .and_then(|ids| {
                    ids.iter().find_map(|id| {
                        ws_opt.and_then(|ws| ws.objects.get(id)).and_then(|objs| {
                            objs.iter().find_map(|o| {
                                o.get("__namespace")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string())
                            })
                        })
                    })
                })
                .unwrap_or_default();

            // The open doc's objects, with namespace backfilled — these are what we scan.
            let iter_objects: Vec<serde_json::Value> = doc
                .objects
                .iter()
                .map(|o| {
                    if doc_ns.is_empty() {
                        return o.clone();
                    }
                    let mut o = o.clone();
                    if let Some(map) = o.as_object_mut() {
                        let needs = map
                            .get("__namespace")
                            .and_then(|v| v.as_str())
                            .map(|s| s.is_empty())
                            .unwrap_or(true);
                        if needs {
                            map.insert("__namespace".to_string(), serde_json::json!(doc_ns));
                        }
                    }
                    o
                })
                .collect();

            // The open file's path as stored in the index (`__file` is project_root-
            // relative). We rebuild this file's objects from the live buffer below, so the
            // STALE indexed copy must be dropped first — otherwise every object defined in
            // the open file appears twice in the resolution index (indexed + freshly
            // parsed), and any same-file `[[#local_id]]` reference looks ambiguous (a false
            // QMDC002). The CLI validator never hits this because it indexes each object once.
            let open_file: Option<String> = ws_opt
                .and_then(|ws| ws.file_to_ids.get(uri.as_str()))
                .and_then(|ids| {
                    ids.iter().find_map(|id| {
                        ws_opt.and_then(|ws| ws.objects.get(id)).and_then(|objs| {
                            objs.iter().find_map(|o| {
                                o.get("__file")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                            })
                        })
                    })
                });

            // Resolution index = (whole workspace minus the open file's stale copy)
            //                     ∪ the open doc's freshly-parsed (namespace-backfilled) objects.
            let mut index_objects: Vec<serde_json::Value> = ws_opt
                .map(|ws| {
                    ws.objects
                        .values()
                        .flatten()
                        .filter(|o| match &open_file {
                            Some(f) => o.get("__file").and_then(|v| v.as_str()) != Some(f.as_str()),
                            None => true,
                        })
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            index_objects.extend(iter_objects.iter().cloned());

            for issue in
                crate::core::ops::validate::collect_reference_issues(&index_objects, &iter_objects)
            {
                let line = (issue.line.max(1) as u32) - 1; // parser 1-based → LSP 0-based
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position {
                            line,
                            character: issue.start_col,
                        },
                        end: Position {
                            line,
                            character: issue.end_col,
                        },
                    },
                    severity: Some(
                        if issue.severity == crate::core::ops::validate::SEVERITY_WARNING {
                            DiagnosticSeverity::WARNING
                        } else {
                            DiagnosticSeverity::ERROR
                        },
                    ),
                    code: Some(NumberOrString::String(issue.code.to_string())),
                    source: Some("qmdc".to_string()),
                    message: issue.message,
                    ..Default::default()
                });
            }
        }

        diagnostics
    }

    /// Extract the actual ID from a reference target
    /// e.g., "#user" -> "user", "User.admin" -> "admin", "auth.user" -> "user"
    pub(crate) fn extract_id_from_target(&self, target: &str) -> String {
        // Single source of truth: delegate to the shared core resolver.
        crate::core::resolve::extract_id_from_target(target)
    }

    /// Find reference at cursor position using __references data
    pub(crate) fn find_reference_at_position<'a>(
        &self,
        doc: &'a Document,
        position: Position,
    ) -> Option<&'a ParsedReference> {
        let line = position.line + 1; // LSP uses 0-based, parser uses 1-based
        let char = position.character;

        doc.references
            .iter()
            .find(|r| r.line == line && char >= r.start_col && char <= r.end_col)
    }

    /// Find Kind if cursor is on Kind part of a definition [[id:Kind]]
    pub(crate) fn find_kind_at_position(
        &self,
        doc: &Document,
        position: Position,
    ) -> Option<String> {
        let lines: Vec<&str> = doc.content.lines().collect();
        let line_content = lines.get(position.line as usize)?;
        let char_pos = utf16_offset_to_byte_offset(line_content, position.character);

        // Look for [[id:Kind]] or [[id: Kind]] pattern
        // Find the bracket pattern containing cursor position
        let mut start_bracket = None;
        let mut search_pos = 0;
        while let Some(pos) = line_content[search_pos..].find("[[") {
            let abs_pos = search_pos + pos;
            if let Some(end_pos) = line_content[abs_pos..].find("]]") {
                let end_abs = abs_pos + end_pos + 2;
                if char_pos >= abs_pos && char_pos < end_abs {
                    start_bracket = Some((abs_pos, end_abs));
                    break;
                }
            }
            search_pos = abs_pos + 2;
        }

        let (start, end) = start_bracket?;
        let bracket_content = &line_content[start + 2..end - 2]; // Content between [[ and ]]

        // Check if there's a colon (Kind separator)
        if let Some(colon_pos) = bracket_content.find(':') {
            let kind_part = bracket_content[colon_pos + 1..].trim();

            // Check if cursor is in the Kind part (after colon)
            let kind_start_in_line = start + 2 + colon_pos + 1;
            let kind_start_trimmed =
                kind_start_in_line + (bracket_content[colon_pos + 1..].len() - kind_part.len());
            let kind_end_in_line = end - 2;

            if char_pos >= kind_start_trimmed && char_pos <= kind_end_in_line {
                return Some(kind_part.to_string());
            }
        }

        None
    }

    /// Find ID if cursor is on ID part of a definition [[id]] or [[id:Kind]]
    /// Returns (id, range of the full definition line)
    /// Only works for headings (lines starting with #)
    pub(crate) fn find_id_in_definition_at_position(
        &self,
        doc: &Document,
        position: Position,
    ) -> Option<(String, Range)> {
        let lines: Vec<&str> = doc.content.lines().collect();
        let line_content = lines.get(position.line as usize)?;
        let char_pos = utf16_offset_to_byte_offset(line_content, position.character);

        // Only process headings (object definitions)
        if !line_content.trim_start().starts_with('#') {
            return None;
        }

        // Look for [[id]] or [[id:Kind]] pattern
        // Find the bracket pattern containing cursor position
        let mut start_bracket = None;
        let mut search_pos = 0;
        while let Some(pos) = line_content[search_pos..].find("[[") {
            let abs_pos = search_pos + pos;
            if let Some(end_pos) = line_content[abs_pos..].find("]]") {
                let end_abs = abs_pos + end_pos + 2;
                if char_pos >= abs_pos && char_pos < end_abs {
                    start_bracket = Some((abs_pos, end_abs));
                    break;
                }
            }
            search_pos = abs_pos + 2;
        }

        let (start, end) = start_bracket?;
        let bracket_content = &line_content[start + 2..end - 2]; // Content between [[ and ]]

        // Skip if this is a reference (starts with #)
        if bracket_content.starts_with('#') {
            return None;
        }

        // Get ID part (before colon if exists)
        let id_part = if let Some(colon_pos) = bracket_content.find(':') {
            bracket_content[..colon_pos].trim()
        } else {
            bracket_content.trim()
        };

        // Check if cursor is in the ID part (before colon or entire bracket if no colon)
        let id_start_in_line = start + 2;
        let id_end_in_line = if let Some(colon_pos) = bracket_content.find(':') {
            start + 2 + colon_pos
        } else {
            end - 2
        };

        if char_pos >= id_start_in_line && char_pos <= id_end_in_line {
            // Return the full line range for the definition
            let line_len = byte_offset_to_utf16_offset(line_content, line_content.len());
            return Some((
                id_part.to_string(),
                Range {
                    start: Position {
                        line: position.line,
                        character: 0,
                    },
                    end: Position {
                        line: position.line,
                        character: line_len,
                    },
                },
            ));
        }

        None
    }

    /// Find all objects with the given __kind across all workspace files
    pub(crate) async fn find_all_objects_by_kind(
        &self,
        kind: &str,
        _include_declaration: bool,
    ) -> Result<Option<Vec<Location>>> {
        let mut locations = Vec::new();
        let mut seen_uris: std::collections::HashSet<Url> = std::collections::HashSet::new();

        // Search in all workspaces
        let workspaces = self.workspaces.read().await;
        for ws in workspaces.by_uri.values() {
            for file_path in &ws.files {
                if let Ok(content) = std::fs::read_to_string(file_path) {
                    let file_uri = Url::from_file_path(file_path).unwrap_or_else(|_| {
                        Url::parse(&format!("file://{}", file_path.display())).unwrap()
                    });

                    let doc = self.parse_and_index(&content, Some(ws.id.clone()));

                    for obj in &doc.objects {
                        if let Some(obj_kind) = obj.get("__kind").and_then(|v| v.as_str()) {
                            if obj_kind == kind {
                                if let Some(line_num) = obj.get("__line").and_then(|v| v.as_u64()) {
                                    let line = line_num as u32 - 1;
                                    let lines: Vec<&str> = content.lines().collect();
                                    let line_content = lines.get(line as usize).unwrap_or(&"");

                                    locations.push(Location {
                                        uri: file_uri.clone(),
                                        range: Range {
                                            start: Position { line, character: 0 },
                                            end: Position {
                                                line,
                                                character: byte_offset_to_utf16_offset(
                                                    line_content,
                                                    line_content.len(),
                                                ),
                                            },
                                        },
                                    });
                                    seen_uris.insert(file_uri.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Also search in open documents that might not be in workspace yet
        let docs = self.documents.read().await;
        for (doc_uri, doc) in docs.iter() {
            // Skip if already found in workspace
            if seen_uris.contains(doc_uri) {
                continue;
            }

            for obj in &doc.objects {
                if let Some(obj_kind) = obj.get("__kind").and_then(|v| v.as_str()) {
                    if obj_kind == kind {
                        if let Some(line_num) = obj.get("__line").and_then(|v| v.as_u64()) {
                            let line = line_num as u32 - 1;
                            let lines: Vec<&str> = doc.content.lines().collect();
                            let line_content = lines.get(line as usize).unwrap_or(&"");

                            locations.push(Location {
                                uri: doc_uri.clone(),
                                range: Range {
                                    start: Position { line, character: 0 },
                                    end: Position {
                                        line,
                                        character: byte_offset_to_utf16_offset(
                                            line_content,
                                            line_content.len(),
                                        ),
                                    },
                                },
                            });
                        }
                    }
                }
            }
        }

        // Sort by URI for consistent ordering
        locations.sort_by(|a, b| a.uri.as_str().cmp(b.uri.as_str()));

        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locations))
        }
    }

    pub(crate) fn find_object_by_id<'a>(
        &self,
        doc: &'a Document,
        id: &str,
    ) -> Option<&'a serde_json::Value> {
        // Try exact match first
        if let Some(&idx) = doc.id_to_object.get(id) {
            return Some(&doc.objects[idx]);
        }
        // Try matching by suffix (for Kind.id or ns.id patterns)
        for (obj_id, &idx) in &doc.id_to_object {
            if obj_id.ends_with(&format!(".{}", id)) || obj_id == id {
                return Some(&doc.objects[idx]);
            }
        }
        None
    }

    pub(crate) fn complete_ids_from_objects<'a>(
        &self,
        objects: impl Iterator<Item = &'a serde_json::Value>,
        partial: &str,
    ) -> Vec<CompletionItem> {
        let partial_lower = partial.to_lowercase();

        let items: Vec<CompletionItem> = objects
            .filter_map(|obj| {
                let id = obj.get("__id").and_then(|v| v.as_str())?;

                // Use __kind, but fall back to 'kind' property if __kind is __Object
                let meta_kind = obj
                    .get("__kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("__Object");
                let kind = if meta_kind == "__Object" {
                    obj.get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("__Object")
                } else {
                    meta_kind
                };

                // Skip system objects (auto-generated IDs like doc_xxx, text_xxx)
                if id.starts_with("doc_") || id.starts_with("text_") {
                    return None;
                }

                let id_lower = id.to_lowercase();

                if !partial.is_empty() {
                    let matches =
                        id_lower.starts_with(&partial_lower) || id_lower.contains(&partial_lower);
                    if !matches {
                        return None;
                    }
                }

                let label_text = obj.get("__label").and_then(|v| v.as_str()).unwrap_or(id);
                let file = obj.get("__file").and_then(|v| v.as_str());

                let doc_str = if let Some(f) = file {
                    format!("{} ({})", label_text, f)
                } else {
                    label_text.to_string()
                };

                Some(CompletionItem {
                    label: id.to_string(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(kind.to_string()),
                    documentation: Some(Documentation::String(doc_str)),
                    ..Default::default()
                })
            })
            .collect();

        items
    }

    pub(crate) fn complete_ids(
        &self,
        doc: &Document,
        partial: &str,
    ) -> Result<Option<CompletionResponse>> {
        let mut items = self.complete_ids_from_objects(doc.objects.iter(), partial);

        // Fallback to fuzzy match if no results
        if items.is_empty() && !partial.is_empty() {
            let partial_lower = partial.to_lowercase();
            items = doc
                .objects
                .iter()
                .filter_map(|obj| {
                    let id = obj.get("__id").and_then(|v| v.as_str())?;
                    let kind = obj
                        .get("__kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("__Object");

                    // Skip system objects (auto-generated IDs)
                    if id.starts_with("doc_") || id.starts_with("text_") {
                        return None;
                    }

                    let id_lower = id.to_lowercase();

                    if !self.fuzzy_match(&partial_lower, &id_lower) {
                        return None;
                    }

                    let label_text = obj.get("__label").and_then(|v| v.as_str()).unwrap_or(id);

                    Some(CompletionItem {
                        label: id.to_string(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(kind.to_string()),
                        documentation: Some(Documentation::String(label_text.to_string())),
                        ..Default::default()
                    })
                })
                .collect();
        }

        items.sort_by(|a, b| a.label.cmp(&b.label));
        Ok(Some(CompletionResponse::Array(items)))
    }

    pub(crate) fn complete_with_kind_filter_from_objects<'a>(
        &self,
        objects: impl Iterator<Item = &'a serde_json::Value>,
        kind_or_ns: &str,
        partial: &str,
    ) -> Vec<CompletionItem> {
        let partial_lower = partial.to_lowercase();
        let kind_lower = kind_or_ns.to_lowercase();

        objects
            .filter_map(|obj| {
                let id = obj.get("__id").and_then(|v| v.as_str())?;
                let kind = obj
                    .get("__kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("__Object");

                // Check if object matches the Kind/namespace filter
                // Either: __kind matches, or id starts with "Kind." (namespace)
                let kind_matches = kind.to_lowercase() == kind_lower;
                let ns_matches = id.to_lowercase().starts_with(&format!("{}.", kind_lower));

                if !kind_matches && !ns_matches {
                    return None;
                }

                // Extract the id part after namespace prefix if present
                let display_id = if ns_matches && id.contains('.') {
                    id.rsplit('.').next().unwrap_or(id)
                } else {
                    id
                };

                // Filter by partial match (prefix or contains, no fuzzy for kind filter)
                if !partial.is_empty() {
                    let display_lower = display_id.to_lowercase();
                    let matches = display_lower.starts_with(&partial_lower)
                        || display_lower.contains(&partial_lower);
                    if !matches {
                        return None;
                    }
                }

                let label_text = obj.get("__label").and_then(|v| v.as_str()).unwrap_or(id);

                Some(CompletionItem {
                    label: display_id.to_string(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(kind.to_string()),
                    documentation: Some(Documentation::String(label_text.to_string())),
                    ..Default::default()
                })
            })
            .collect()
    }

    pub(crate) fn complete_with_kind_filter(
        &self,
        doc: &Document,
        kind_or_ns: &str,
        partial: &str,
    ) -> Result<Option<CompletionResponse>> {
        let mut items =
            self.complete_with_kind_filter_from_objects(doc.objects.iter(), kind_or_ns, partial);

        items.sort_by(|a, b| a.label.cmp(&b.label));
        Ok(Some(CompletionResponse::Array(items)))
    }

    pub(crate) fn complete_kinds(&self, doc: &Document) -> Result<Option<CompletionResponse>> {
        self.complete_kinds_from_objects(doc.objects.iter())
    }

    pub(crate) fn complete_kinds_from_objects<'a, I>(
        &self,
        objects: I,
    ) -> Result<Option<CompletionResponse>>
    where
        I: Iterator<Item = &'a serde_json::Value>,
    {
        let mut kinds: Vec<String> = objects
            .filter_map(|obj| {
                obj.get("__kind")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .filter(|k| !k.is_empty())
            // Filter out system kinds except __Object
            .filter(|k| k == "__Object" || !k.starts_with("__"))
            .collect();
        kinds.sort();
        kinds.dedup();

        // Always include __Object
        if !kinds.contains(&"__Object".to_string()) {
            kinds.push("__Object".to_string());
            kinds.sort();
        }

        let items: Vec<CompletionItem> = kinds
            .into_iter()
            .map(|kind| CompletionItem {
                label: kind,
                kind: Some(CompletionItemKind::CLASS),
                ..Default::default()
            })
            .collect();

        Ok(Some(CompletionResponse::Array(items)))
    }

    /// Fuzzy match: all characters of pattern appear in target in order
    pub(crate) fn fuzzy_match(&self, pattern: &str, target: &str) -> bool {
        let mut pattern_chars = pattern.chars().peekable();
        for c in target.chars() {
            if pattern_chars.peek() == Some(&c) {
                pattern_chars.next();
            }
        }
        pattern_chars.peek().is_none()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Initialize workspaces from VS Code workspace folders
        self.init_workspaces(params.workspace_folders).await;

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "[".to_string(),
                        "#".to_string(),
                        ":".to_string(),
                        ".".to_string(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["qmdc.dumpIndex".to_string()],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let ws_index = self.workspaces.read().await;
        let ws_count = ws_index.by_uri.len();
        drop(ws_index);

        // Dynamically register a file-system watcher for QMD.md files so the client
        // reliably notifies us of create/change/delete events anywhere in the
        // workspace. Without this we depend on the client volunteering
        // didChangeWatchedFiles, and a newly-created target file might never be
        // observed — leaving stale broken_link diagnostics until restart.
        // See QMD-58.
        //
        // register_capability is a server->client request; do not block
        // `initialized` waiting for the client's reply (some clients are slow or
        // do not support dynamic registration). Fire it off in the background.
        let client = self.client.clone();
        tokio::spawn(async move {
            let registration = Registration {
                id: "qmdc-watch-files".to_string(),
                method: "workspace/didChangeWatchedFiles".to_string(),
                register_options: serde_json::to_value(DidChangeWatchedFilesRegistrationOptions {
                    watchers: vec![FileSystemWatcher {
                        glob_pattern: GlobPattern::String("**/*.qmd.md".to_string()),
                        kind: None, // None = create | change | delete
                    }],
                })
                .ok(),
            };
            if let Err(e) = client.register_capability(vec![registration]).await {
                eprintln!("[LSP] Failed to register file watcher: {:?}", e);
            }
        });

        self.client
            .log_message(
                MessageType::INFO,
                format!("QMDC LSP initialized with {} workspace(s)", ws_count),
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        match params.command.as_str() {
            "qmdc.dumpIndex" => {
                commands::handle_dump_index(&self.client, &self.workspaces, &self.documents).await
            }
            "qmdc.getWorkspaceTree" => {
                // Arguments: [".", "mode"] or ["mode"] (backward compatibility)
                // Take last argument as mode (supports both formats)
                let grouping_mode = params
                    .arguments
                    .last()
                    .and_then(|v| v.as_str())
                    .unwrap_or("namespace");

                self.sync_sqlite().await;

                // Get result from database (lock is released after this block)
                let mut result = {
                    let db = self.db.lock().map_err(|e| {
                        tower_lsp::jsonrpc::Error::invalid_params(format!("DB lock error: {}", e))
                    })?;
                    commands::handle_get_workspace_tree(&db, grouping_mode)?
                };

                // Enrich workspace objects with projectRoot from WorkspaceInfo
                // (db lock is released, so we can await here)
                if let Some(result_value) = result.as_mut() {
                    if let Some(workspaces) = result_value
                        .get_mut("workspaces")
                        .and_then(|w| w.as_array_mut())
                    {
                        let ws_index = self.workspaces.read().await;
                        for ws in workspaces {
                            if let Some(ws_id) = ws.get("id").and_then(|v| v.as_str()) {
                                // Find WorkspaceInfo by ID and add project_root
                                if let Some(ws_info) = ws_index.get_by_id(ws_id) {
                                    if let Some(ws_obj) = ws.as_object_mut() {
                                        ws_obj.insert(
                                            "projectRoot".to_string(),
                                            serde_json::json!(ws_info
                                                .project_root
                                                .to_string_lossy()),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(result)
            }
            "qmdc.runSqlQuery" => {
                let query_input = params
                    .arguments
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let document_uri = params.arguments.get(1).and_then(|v| v.as_str());

                let scope = params.arguments.get(2).and_then(|v| v.as_str());

                self.sync_sqlite().await;

                // Get workspace index first (async, Send)
                let ws_index = self.workspaces.read().await;

                // Then get DB lock (sync, not Send, but we don't await after this)
                let db = self.db.lock().map_err(|e| {
                    tower_lsp::jsonrpc::Error::invalid_params(format!("DB lock error: {}", e))
                })?;

                commands::handle_run_sql_query(&db, query_input, document_uri, scope, &ws_index)
            }
            _ => Ok(None),
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = params.text_document.text;

        // Find which workspace this file belongs to
        let ws_id = {
            let ws_index = self.workspaces.read().await;
            self.find_workspace_for_file(&uri, &ws_index)
                .map(|ws| ws.id.clone())
        };

        let doc = self.parse_and_index(&content, ws_id);

        self.publish_diagnostics(uri.clone(), &doc).await;

        {
            let mut docs = self.documents.write().await;
            docs.insert(uri, doc);
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().next() {
            // Find which workspace this file belongs to
            let ws_id = {
                let ws_index = self.workspaces.read().await;
                self.find_workspace_for_file(&uri, &ws_index)
                    .map(|ws| ws.id.clone())
            };

            let doc = self.parse_and_index(&change.text, ws_id);

            // Update workspace objects index with new objects from this document
            {
                let mut ws_index = self.workspaces.write().await;
                ws_index.update_objects_from_document(uri.as_str(), &doc);
            }
            self.db_dirty
                .store(true, std::sync::atomic::Ordering::Release);

            self.publish_diagnostics(uri.clone(), &doc).await;

            {
                let mut docs = self.documents.write().await;
                docs.insert(uri, doc);
            }

            // NOTE: we intentionally do NOT refresh OTHER open documents here.
            // While typing an anchor id the id-set changes on nearly every
            // keystroke, so a cross-file refresh per `did_change` would re-publish
            // all open docs per character. The create-then-define authoring flow
            // (QMD-58) is covered by `did_save` and `did_change_watched_files`,
            // which fire at natural, infrequent boundaries.
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        eprintln!("[LSP] did_save: {}", uri);

        // Check if this is a structural file (readme.qmd.md, case-insensitive)
        let is_readme = Self::is_readme_file(uri.path());

        if is_readme {
            // readme.qmd.md might have changed workspace/namespace structure - rescan
            eprintln!("[LSP] did_save: readme.qmd.md changed, rescanning...");
            self.rescan_workspaces().await;
            return;
        }

        // Regular file - incremental update
        // Snapshot the ids this document defined before the save, to detect
        // anchors added/removed that other open documents reference. See QMD-58.
        let old_ids = {
            let docs = self.documents.read().await;
            docs.get(&uri).map(Self::object_ids).unwrap_or_default()
        };
        let mut ids_changed = false;

        // If content is provided, re-index
        if let Some(text) = params.text {
            let ws_id = {
                let ws_index = self.workspaces.read().await;
                self.find_workspace_for_file(&uri, &ws_index)
                    .map(|ws| ws.id.clone())
            };

            let doc = self.parse_and_index(&text, ws_id);
            ids_changed = Self::object_ids(&doc) != old_ids;

            // Update workspace objects index
            {
                let mut ws_index = self.workspaces.write().await;
                ws_index.update_objects_from_document(uri.as_str(), &doc);
            }
            self.db_dirty
                .store(true, std::sync::atomic::Ordering::Release);

            self.publish_diagnostics(uri.clone(), &doc).await;

            {
                let mut docs = self.documents.write().await;
                docs.insert(uri.clone(), doc);
            }
        } else {
            // No content provided - re-read from disk and re-index
            if let Ok(path) = uri.to_file_path() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    let ws_id = {
                        let ws_index = self.workspaces.read().await;
                        self.find_workspace_for_file(&uri, &ws_index)
                            .map(|ws| ws.id.clone())
                    };

                    let doc = self.parse_and_index(&text, ws_id);
                    ids_changed = Self::object_ids(&doc) != old_ids;

                    // Update workspace objects index
                    {
                        let mut ws_index = self.workspaces.write().await;
                        ws_index.update_objects_from_document(uri.as_str(), &doc);
                    }
                    self.db_dirty
                        .store(true, std::sync::atomic::Ordering::Release);

                    self.publish_diagnostics(uri.clone(), &doc).await;

                    {
                        let mut docs = self.documents.write().await;
                        docs.insert(uri.clone(), doc);
                    }
                }
            }
        }

        // If this save changed the set of defined ids, references to those ids
        // in OTHER open documents may now resolve (or newly break) — re-publish
        // their diagnostics so the editor matches the CLI validator. See QMD-58.
        if ids_changed {
            self.refresh_other_open_documents(&uri).await;
        }

        // Sync SQLite database so Explorer gets updated data
        eprintln!("[LSP] did_save: syncing SQLite...");
        self.sync_sqlite().await;
        eprintln!("[LSP] did_save: SQLite synced");

        // Notify extension that workspace was updated
        self.client
            .send_notification::<WorkspaceUpdated>(WorkspaceUpdatedParams {})
            .await;
        eprintln!("[LSP] did_save: notification sent");
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut docs = self.documents.write().await;
        docs.remove(&params.text_document.uri);
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let mut actions = Vec::new();

        for diag in &params.context.diagnostics {
            // Check if this is a QMDC001 diagnostic with a "Did you mean" hint
            if let Some(NumberOrString::String(code)) = &diag.code {
                if code == "QMDC001" && diag.message.contains("Did you mean [[#") {
                    // Extract the suggested reference from the message
                    if let Some(start) = diag.message.find("[[#") {
                        if let Some(end) = diag.message[start..].find("]]") {
                            let suggested = &diag.message[start..start + end + 2];
                            // The diagnostic range covers the broken reference in the source
                            // We need to replace it with the suggested one
                            let edit = TextEdit {
                                range: diag.range,
                                new_text: suggested.to_string(),
                            };
                            let mut changes = std::collections::HashMap::new();
                            changes.insert(params.text_document.uri.clone(), vec![edit]);
                            let workspace_edit = WorkspaceEdit {
                                changes: Some(changes),
                                ..Default::default()
                            };
                            let action = CodeAction {
                                title: format!("Replace with {}", suggested),
                                kind: Some(CodeActionKind::QUICKFIX),
                                diagnostics: Some(vec![diag.clone()]),
                                edit: Some(workspace_edit),
                                ..Default::default()
                            };
                            actions.push(CodeActionOrCommand::CodeAction(action));
                        }
                    }
                }
            }
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        eprintln!(
            "[LSP] did_change_watched_files: {} changes",
            params.changes.len()
        );

        // Save changes to avoid move issues
        let changes = params.changes;

        let mut needs_rescan = false;

        for change in &changes {
            let uri = &change.uri;
            eprintln!("[LSP] File change: {:?} - {}", change.typ, uri);

            // Only process .qmd.md files
            if !uri.path().ends_with(".qmd.md") {
                continue;
            }

            // Detect structural changes that require full rescan
            let is_structural_change = match change.typ {
                FileChangeType::CREATED => {
                    // New file created - could be new workspace/namespace
                    true
                }
                FileChangeType::DELETED => {
                    // File deleted - workspace/namespace might be gone
                    true
                }
                FileChangeType::CHANGED => {
                    // Check if it's readme.qmd.md (might change workspace/namespace structure, case-insensitive)
                    Self::is_readme_file(uri.path())
                }
                _ => false,
            };

            if is_structural_change {
                eprintln!(
                    "[LSP] Structural change detected: {:?} - {}",
                    change.typ, uri
                );
                needs_rescan = true;
            }
        }

        if needs_rescan {
            // Full rescan for structural changes
            self.rescan_workspaces().await;
        } else {
            // Incremental update for content changes
            let mut needs_resync = false;
            let mut ids_changed = false;

            for change in changes {
                let uri = change.uri;

                if !uri.path().ends_with(".qmd.md") {
                    continue;
                }

                if change.typ == FileChangeType::CHANGED {
                    // Re-read and re-index the file
                    if let Ok(path) = uri.to_file_path() {
                        if let Ok(text) = std::fs::read_to_string(&path) {
                            let ws_id = {
                                let ws_index = self.workspaces.read().await;
                                self.find_workspace_for_file(&uri, &ws_index)
                                    .map(|ws| ws.id.clone())
                            };

                            // Track whether this external change altered the set
                            // of object ids the file defines (anchors added or
                            // removed), so we only do the cross-file refresh when
                            // it can actually affect other docs' references.
                            let old_ids = {
                                let docs = self.documents.read().await;
                                docs.get(&uri).map(Self::object_ids).unwrap_or_default()
                            };

                            let doc = self.parse_and_index(&text, ws_id);
                            if Self::object_ids(&doc) != old_ids {
                                ids_changed = true;
                            }

                            // Update workspace objects index
                            {
                                let mut ws_index = self.workspaces.write().await;
                                ws_index.update_objects_from_document(uri.as_str(), &doc);
                            }
                            self.db_dirty
                                .store(true, std::sync::atomic::Ordering::Release);

                            self.publish_diagnostics(uri.clone(), &doc).await;

                            {
                                let mut docs = self.documents.write().await;
                                docs.insert(uri, doc);
                            }

                            needs_resync = true;
                        }
                    }
                }
            }

            if needs_resync {
                eprintln!("[LSP] did_change_watched_files: syncing SQLite...");
                self.sync_sqlite().await;
                eprintln!("[LSP] did_change_watched_files: SQLite synced");

                // External content changes that added/removed anchors may make
                // references in OTHER open documents resolve (or newly break) —
                // re-publish open docs so the editor matches the CLI validator.
                // Guarded on an id-set change to avoid needless refreshes when
                // only field values changed. See QMD-58.
                if ids_changed {
                    self.refresh_open_documents().await;
                }

                // Notify extension
                self.client
                    .send_notification::<WorkspaceUpdated>(WorkspaceUpdatedParams {})
                    .await;
                eprintln!("[LSP] did_change_watched_files: notification sent");
            }
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        super::handlers::completion::handle(self, params).await
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        super::handlers::hover::handle(self, params).await
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        super::handlers::definition::handle(self, params).await
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        super::handlers::references::handle(self, params).await
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        super::handlers::document_symbol::handle(self, params).await
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        super::handlers::workspace_symbol::handle(self, params).await
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        super::handlers::rename::handle_prepare(self, params).await
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        super::handlers::rename::handle(self, params).await
    }
}

pub async fn run_lsp() {
    eprintln!("[qmdc] LSP server starting...");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);

    eprintln!("[qmdc] LSP server ready, waiting for messages...");

    Server::new(stdin, stdout, socket).serve(service).await;

    eprintln!("[qmdc] LSP server shutting down");
}
