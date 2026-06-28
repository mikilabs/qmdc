use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::Url;

use super::document::Document;

/// Information about a QMDC workspace
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WorkspaceInfo {
    /// Workspace ID from [[id:__Workspace]]
    pub id: String,
    /// Root folder URI (workspace folder, e.g. /project/docs/)
    pub root_uri: Url,
    /// Root folder path (workspace folder, e.g. /project/docs/)
    pub root_path: PathBuf,
    /// Project root path (VSCode workspace folder, e.g. /project/)
    pub project_root: PathBuf,
    /// All QMD.md files in this workspace
    pub files: Vec<PathBuf>,
    /// All objects indexed by ID (may have multiple objects with same ID in different namespaces)
    pub objects: HashMap<String, Vec<serde_json::Value>>,
    /// All objects indexed by __local_id (for fallback resolution)
    pub by_local_id: HashMap<String, Vec<serde_json::Value>>,
    /// Track which object IDs belong to which file (for cleanup on change)
    pub file_to_ids: HashMap<String, Vec<String>>,
    /// Line where workspace is defined (for diagnostics)
    pub def_line: u32,
}

/// Index of all workspaces for cross-workspace resolution
#[derive(Debug, Default)]
pub struct WorkspaceIndex {
    /// workspace_id -> Vec<WorkspaceInfo> (vector because IDs might duplicate)
    pub by_id: HashMap<String, Vec<WorkspaceInfo>>,
    /// folder_uri -> WorkspaceInfo (always unique)
    pub by_uri: HashMap<Url, WorkspaceInfo>,
}

#[allow(dead_code)]
impl WorkspaceIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a workspace ID is ambiguous (duplicated)
    pub fn is_ambiguous(&self, id: &str) -> bool {
        self.by_id.get(id).map(|v| v.len() > 1).unwrap_or(false)
    }

    /// Get workspace by ID (returns None if ambiguous or not found)
    pub fn get_by_id(&self, id: &str) -> Option<&WorkspaceInfo> {
        self.by_id
            .get(id)
            .and_then(|v| if v.len() == 1 { Some(&v[0]) } else { None })
    }

    /// Get all duplicate workspace IDs
    pub fn get_duplicates(&self) -> Vec<(&str, &[WorkspaceInfo])> {
        self.by_id
            .iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(k, v)| (k.as_str(), v.as_slice()))
            .collect()
    }

    /// Add a workspace to the index
    pub fn add(&mut self, info: WorkspaceInfo) {
        let id = info.id.clone();
        let uri = info.root_uri.clone();

        self.by_id.entry(id).or_default().push(info.clone());
        self.by_uri.insert(uri, info);
    }

    /// Update objects in workspace when a document changes
    pub fn update_objects_from_document(&mut self, file_uri: &str, doc: &Document) {
        let new_ids: Vec<String> = doc
            .objects
            .iter()
            .filter_map(|obj| {
                obj.get("__id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        let file_url = Url::parse(file_uri).ok();
        let file_path = file_url.as_ref().and_then(|u| u.to_file_path().ok());

        for ws in self.by_uri.values_mut() {
            let is_in_workspace = file_path
                .as_ref()
                .map(|p| p.starts_with(&ws.root_path))
                .unwrap_or(false);

            if !is_in_workspace {
                continue;
            }

            let relative_path = file_path.as_ref().and_then(|p| {
                p.strip_prefix(&ws.project_root)
                    .ok()
                    .map(|r| r.to_string_lossy().to_string())
            });

            // STEP 1: Save namespace from OLD objects BEFORE removing them
            // This is the safest way - if object already had namespace, preserve it
            let mut saved_namespaces: HashMap<String, String> = HashMap::new();
            if let Some(old_ids) = ws.file_to_ids.get(file_uri) {
                for old_id in old_ids {
                    if let Some(objs) = ws.objects.get(old_id) {
                        if let Some(obj) = objs.first() {
                            if let Some(ns) = obj.get("__namespace").and_then(|v| v.as_str()) {
                                if !ns.is_empty() {
                                    saved_namespaces.insert(old_id.clone(), ns.to_string());
                                }
                            }
                        }
                    }
                }
            }

            // STEP 2: Try to find namespace from file path (walking up directory tree)
            // This doesn't depend on ws.objects state
            let namespace_from_path = Self::find_namespace_from_path(
                file_path.as_deref(),
                &ws.root_path,
                &ws.project_root,
                &ws.objects,
            );

            // STEP 3: Remove old objects (from ws.objects and ws.by_local_id)
            if let Some(old_ids) = ws.file_to_ids.get(file_uri) {
                for old_id in old_ids {
                    ws.objects.remove(old_id);
                }
            }

            // Remove stale entries from by_local_id for this file
            let rel_path_for_cleanup = relative_path.clone();
            if let Some(ref rel_path_str) = rel_path_for_cleanup {
                ws.by_local_id.retain(|_local_id, objs| {
                    objs.retain(|obj| {
                        obj.get("__file")
                            .and_then(|v| v.as_str())
                            .map(|f| f != rel_path_str.as_str())
                            .unwrap_or(true)
                    });
                    !objs.is_empty()
                });
            }

            // STEP 4: Add new objects with metadata
            for obj in &doc.objects {
                if let Some(id) = obj.get("__id").and_then(|v| v.as_str()) {
                    let mut obj_with_metadata = obj.clone();
                    if let Some(obj_map) = obj_with_metadata.as_object_mut() {
                        if let Some(ref rel_path) = relative_path {
                            obj_map.insert("__file".to_string(), serde_json::json!(rel_path));
                        }
                        obj_map.insert("__workspace".to_string(), serde_json::json!(ws.id.clone()));

                        // Determine namespace with priority:
                        // 1. Object already has __namespace in document -> keep it
                        // 2. Saved namespace from old object with same ID -> restore it
                        // 3. Namespace found from file path -> use it
                        if !obj_map.contains_key("__namespace") {
                            let ns = saved_namespaces
                                .get(id)
                                .cloned()
                                .or_else(|| namespace_from_path.clone());

                            if let Some(ns_id) = ns {
                                obj_map.insert("__namespace".to_string(), serde_json::json!(ns_id));
                            }
                        }
                    }
                    ws.objects
                        .entry(id.to_string())
                        .or_default()
                        .push(obj_with_metadata.clone());

                    // Populate by_local_id for objects with non-empty __local_id
                    if let Some(local_id) =
                        obj_with_metadata.get("__local_id").and_then(|v| v.as_str())
                    {
                        if !local_id.is_empty() {
                            ws.by_local_id
                                .entry(local_id.to_string())
                                .or_default()
                                .push(obj_with_metadata);
                        }
                    }
                }
            }

            ws.file_to_ids.insert(file_uri.to_string(), new_ids.clone());
        }
    }

    /// Find namespace by walking up the directory tree from file path
    /// This is a pure function that doesn't modify state
    fn find_namespace_from_path(
        file_path: Option<&Path>,
        root_path: &Path,
        project_root: &Path,
        objects: &HashMap<String, Vec<serde_json::Value>>,
    ) -> Option<String> {
        let path = file_path?;
        let rel = path.strip_prefix(root_path).ok()?;
        let mut current_dir = rel.parent();

        while let Some(dir) = current_dir {
            if dir.as_os_str().is_empty() {
                break;
            }

            let ns_readme = root_path.join(dir).join("readme.qmd.md");
            if ns_readme.exists() {
                // Find __Namespace object for this folder
                for (id, objs) in objects {
                    for obj in objs {
                        if obj.get("__kind").and_then(|v| v.as_str()) == Some("__Namespace") {
                            if let Some(obj_file) = obj.get("__file").and_then(|v| v.as_str()) {
                                let obj_path = project_root.join(obj_file);
                                if let Some(obj_parent) = obj_path.parent() {
                                    if obj_parent == root_path.join(dir) {
                                        return Some(id.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            current_dir = dir.parent();
        }
        None
    }
}
