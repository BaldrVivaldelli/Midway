use serde::{Deserialize, Serialize};

use super::http::{HttpMethod, KeyValueRow, RequestDraft};

// ─── Folder domain entity (Req 3.1, 3.2, 3.3) ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
}

/// Validation errors for folder operations (Req 3.1, 3.5, 3.6, 3.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderValidationError {
    /// name (after trim) is outside [1, 100] characters.
    NameLength { len: usize },
    /// parent_folder_id creates a cycle (folder is an ancestor of itself).
    Cycle { folder_id: String },
    /// parent_folder_id points to a folder in a different collection.
    CrossCollectionParent,
    /// request is being associated with a folder from a different collection.
    CrossCollectionRequest,
    /// parent_folder_id references a non-existent folder.
    ParentNotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    pub id: String,
    pub name: String,
    pub request_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedRequestRecord {
    pub id: String,
    pub collection_id: String,
    #[serde(default)]
    pub folder_id: Option<String>,
    pub name: String,
    pub draft: RequestDraft,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionWithRequests {
    pub collection: CollectionSummary,
    #[serde(default)]
    pub folders: Vec<Folder>,
    pub requests: Vec<SavedRequestRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequestInput {
    pub request_id: Option<String>,
    pub collection_id: String,
    #[serde(default)]
    pub folder_id: Option<String>,
    pub draft: RequestDraft,
}

#[derive(Debug, Clone)]
pub struct SaveFolderInput {
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentRecord {
    pub id: String,
    pub name: String,
    pub variables: Vec<KeyValueRow>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEnvironmentInput {
    pub environment_id: Option<String>,
    pub name: String,
    pub variables: Vec<KeyValueRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMetadata {
    pub alias: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSecretInput {
    pub alias: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub request_name: String,
    pub method: HttpMethod,
    pub url: String,
    pub environment_name: Option<String>,
    pub response_status: Option<u16>,
    pub duration_ms: Option<u64>,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub collections: Vec<CollectionWithRequests>,
    pub environments: Vec<EnvironmentRecord>,
    pub history: Vec<HistoryEntry>,
    pub secrets: Vec<SecretMetadata>,
}

// ─── Pure validation functions (Req 3.1, 3.2, 3.5, 3.6, 3.8) ─────────────────

/// Validates that `name.trim()` has between 1 and 100 characters (Req 3.1).
pub fn validate_folder_name(name: &str) -> Result<(), FolderValidationError> {
    let trimmed_len = name.trim().chars().count();
    if !(1..=100).contains(&trimmed_len) {
        Err(FolderValidationError::NameLength { len: trimmed_len })
    } else {
        Ok(())
    }
}

/// Validates that inserting/moving `folder` (with its `parent_folder_id`) into
/// the set `existing` does not create a cycle or cross-collection reference (Req 3.2, 3.5, 3.6).
/// `existing` is the current set of folders in the collection.
pub fn validate_folder_placement(
    folder: &Folder,
    existing: &[Folder],
) -> Result<(), FolderValidationError> {
    // If no parent, it's a first-level folder — always valid.
    let parent_id = match &folder.parent_folder_id {
        None => return Ok(()),
        Some(id) => id,
    };

    // Find the parent in existing folders.
    let parent = existing
        .iter()
        .find(|f| f.id == *parent_id)
        .ok_or(FolderValidationError::ParentNotFound)?;

    // Check cross-collection: parent must be in the same collection.
    if parent.collection_id != folder.collection_id {
        return Err(FolderValidationError::CrossCollectionParent);
    }

    // Walk up the ancestor chain to detect cycles.
    // If we encounter the folder's own id, it's a cycle.
    // If the chain doesn't terminate within N steps (N = existing.len()), it's a cycle.
    let max_steps = existing.len();
    let mut current_id = parent_id.clone();

    for _ in 0..max_steps {
        // If current ancestor is the folder itself, we have a cycle.
        if current_id == folder.id {
            return Err(FolderValidationError::Cycle {
                folder_id: folder.id.clone(),
            });
        }

        // Find the current ancestor in existing.
        let ancestor = match existing.iter().find(|f| f.id == current_id) {
            Some(a) => a,
            None => {
                // Ancestor chain terminated (parent_folder_id is None or not found),
                // which means no cycle.
                return Ok(());
            }
        };

        // Move up to the next ancestor.
        match &ancestor.parent_folder_id {
            None => return Ok(()), // Reached a root folder, no cycle.
            Some(next_id) => current_id = next_id.clone(),
        }
    }

    // If we exhausted max_steps without terminating, there's a cycle.
    Err(FolderValidationError::Cycle {
        folder_id: folder.id.clone(),
    })
}

/// Validates that a request can only be associated with a folder from its same
/// collection (Req 3.8).
pub fn validate_request_folder_association(
    request_collection_id: &str,
    folder: Option<&Folder>,
) -> Result<(), FolderValidationError> {
    if let Some(f) = folder {
        if f.collection_id != request_collection_id {
            return Err(FolderValidationError::CrossCollectionRequest);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── validate_folder_name tests ────────────────────────────────────────

    #[test]
    fn name_valid_single_char() {
        assert_eq!(validate_folder_name("a"), Ok(()));
    }

    #[test]
    fn name_valid_100_chars() {
        let name = "a".repeat(100);
        assert_eq!(validate_folder_name(&name), Ok(()));
    }

    #[test]
    fn name_length_counts_unicode_characters_instead_of_bytes() {
        let valid_name = "📁".repeat(100);
        assert_eq!(validate_folder_name(&valid_name), Ok(()));

        let invalid_name = "📁".repeat(101);
        assert_eq!(
            validate_folder_name(&invalid_name),
            Err(FolderValidationError::NameLength { len: 101 })
        );
    }

    #[test]
    fn name_valid_with_surrounding_whitespace() {
        assert_eq!(validate_folder_name("  hello  "), Ok(()));
    }

    #[test]
    fn name_empty_after_trim() {
        assert_eq!(
            validate_folder_name("   "),
            Err(FolderValidationError::NameLength { len: 0 })
        );
    }

    #[test]
    fn name_too_long() {
        let name = "a".repeat(101);
        assert_eq!(
            validate_folder_name(&name),
            Err(FolderValidationError::NameLength { len: 101 })
        );
    }

    #[test]
    fn name_empty_string() {
        assert_eq!(
            validate_folder_name(""),
            Err(FolderValidationError::NameLength { len: 0 })
        );
    }

    // ─── validate_folder_placement tests ───────────────────────────────────

    fn make_folder(id: &str, collection_id: &str, parent: Option<&str>) -> Folder {
        Folder {
            id: id.to_string(),
            collection_id: collection_id.to_string(),
            parent_folder_id: parent.map(|s| s.to_string()),
            name: "Test".to_string(),
        }
    }

    #[test]
    fn placement_no_parent_is_valid() {
        let folder = make_folder("f1", "c1", None);
        assert_eq!(validate_folder_placement(&folder, &[]), Ok(()));
    }

    #[test]
    fn placement_parent_not_found() {
        let folder = make_folder("f1", "c1", Some("nonexistent"));
        let existing = vec![make_folder("f2", "c1", None)];
        assert_eq!(
            validate_folder_placement(&folder, &existing),
            Err(FolderValidationError::ParentNotFound)
        );
    }

    #[test]
    fn placement_cross_collection_parent() {
        let parent = make_folder("p1", "c2", None); // different collection
        let folder = make_folder("f1", "c1", Some("p1"));
        let existing = vec![parent];
        assert_eq!(
            validate_folder_placement(&folder, &existing),
            Err(FolderValidationError::CrossCollectionParent)
        );
    }

    #[test]
    fn placement_valid_nested() {
        let grandparent = make_folder("gp", "c1", None);
        let parent = make_folder("p", "c1", Some("gp"));
        let folder = make_folder("f1", "c1", Some("p"));
        let existing = vec![grandparent, parent];
        assert_eq!(validate_folder_placement(&folder, &existing), Ok(()));
    }

    #[test]
    fn placement_direct_cycle() {
        // folder's parent is itself
        let folder = make_folder("f1", "c1", Some("f1"));
        let existing = vec![make_folder("f1", "c1", Some("f1"))];
        assert_eq!(
            validate_folder_placement(&folder, &existing),
            Err(FolderValidationError::Cycle {
                folder_id: "f1".to_string()
            })
        );
    }

    #[test]
    fn placement_indirect_cycle() {
        // f1 -> f2 -> f3 -> f1 (cycle)
        let f2 = make_folder("f2", "c1", Some("f3"));
        let f3 = make_folder("f3", "c1", Some("f1"));
        // f1 wants parent f2, and f2 -> f3 -> f1, so it's a cycle
        let folder = make_folder("f1", "c1", Some("f2"));
        let existing = vec![f2, f3];
        assert_eq!(
            validate_folder_placement(&folder, &existing),
            Err(FolderValidationError::Cycle {
                folder_id: "f1".to_string()
            })
        );
    }

    // ─── validate_request_folder_association tests ─────────────────────────

    #[test]
    fn association_no_folder_is_valid() {
        assert_eq!(validate_request_folder_association("c1", None), Ok(()));
    }

    #[test]
    fn association_same_collection_is_valid() {
        let folder = make_folder("f1", "c1", None);
        assert_eq!(
            validate_request_folder_association("c1", Some(&folder)),
            Ok(())
        );
    }

    #[test]
    fn association_cross_collection_rejected() {
        let folder = make_folder("f1", "c2", None);
        assert_eq!(
            validate_request_folder_association("c1", Some(&folder)),
            Err(FolderValidationError::CrossCollectionRequest)
        );
    }
}
