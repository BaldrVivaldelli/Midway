use midway_core::{
    domain::{
        http::{AuthConfig, BodyMode, HttpMethod, RequestBodyDraft, RequestDraft},
        workspace::{SaveFolderInput, SaveRequestInput},
    },
    infra::sqlite_repository::SqliteRepository,
};

async fn open_temp_repository() -> (SqliteRepository, tempfile::TempDir) {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let repository = SqliteRepository::open(temp_dir.path().join("workspace.sqlite3"))
        .await
        .expect("open repository");
    (repository, temp_dir)
}

fn request_draft(name: &str) -> RequestDraft {
    RequestDraft {
        id: None,
        name: name.to_string(),
        method: HttpMethod::GET,
        url: "https://example.com".to_string(),
        query: Vec::new(),
        headers: Vec::new(),
        auth: AuthConfig::None,
        body: RequestBodyDraft {
            mode: BodyMode::None,
            value: String::new(),
            form_data: Vec::new(),
        },
        timeout_ms: 30_000,
        environment_id: None,
        response_tests: Vec::new(),
    }
}

#[tokio::test]
async fn snapshot_import_preserves_nested_folders_and_request_assignment() {
    let (source, _source_dir) = open_temp_repository().await;
    let collection = source
        .create_collection("Source".to_string())
        .await
        .unwrap();
    let parent = source
        .create_folder(SaveFolderInput {
            collection_id: collection.id.clone(),
            parent_folder_id: None,
            name: "Parent".to_string(),
        })
        .await
        .unwrap();
    let child = source
        .create_folder(SaveFolderInput {
            collection_id: collection.id.clone(),
            parent_folder_id: Some(parent.id.clone()),
            name: "Child".to_string(),
        })
        .await
        .unwrap();
    let request = source
        .save_request(SaveRequestInput {
            request_id: None,
            collection_id: collection.id.clone(),
            folder_id: Some(child.id.clone()),
            draft: request_draft("Nested request"),
        })
        .await
        .unwrap();

    let mut snapshot = source.export_full_snapshot().await.unwrap();
    snapshot.collections[0]
        .folders
        .sort_by_key(|folder| usize::from(folder.id == parent.id));
    assert_eq!(snapshot.collections[0].folders[0].id, child.id);

    let (target, _target_dir) = open_temp_repository().await;
    target
        .import_workspace_snapshot(snapshot, false)
        .await
        .unwrap();

    let imported = target
        .get_collection_with_requests(&collection.id)
        .await
        .unwrap()
        .unwrap();
    let imported_parent = imported
        .folders
        .iter()
        .find(|folder| folder.id == parent.id)
        .unwrap();
    let imported_child = imported
        .folders
        .iter()
        .find(|folder| folder.id == child.id)
        .unwrap();
    let imported_request = imported
        .requests
        .iter()
        .find(|candidate| candidate.id == request.id)
        .unwrap();

    assert_eq!(imported_parent.parent_folder_id, None);
    assert_eq!(
        imported_child.parent_folder_id.as_deref(),
        Some(parent.id.as_str())
    );
    assert_eq!(
        imported_request.folder_id.as_deref(),
        Some(child.id.as_str())
    );
}

#[tokio::test]
async fn invalid_folder_tree_rolls_back_the_entire_snapshot_import() {
    let (source, _source_dir) = open_temp_repository().await;
    let source_collection = source
        .create_collection("Invalid source".to_string())
        .await
        .unwrap();
    let parent = source
        .create_folder(SaveFolderInput {
            collection_id: source_collection.id.clone(),
            parent_folder_id: None,
            name: "Parent".to_string(),
        })
        .await
        .unwrap();
    let child = source
        .create_folder(SaveFolderInput {
            collection_id: source_collection.id.clone(),
            parent_folder_id: Some(parent.id),
            name: "Child".to_string(),
        })
        .await
        .unwrap();

    let mut invalid_snapshot = source.export_full_snapshot().await.unwrap();
    invalid_snapshot.collections[0]
        .folders
        .iter_mut()
        .find(|folder| folder.id == child.id)
        .unwrap()
        .parent_folder_id = Some("missing-parent".to_string());

    let (target, _target_dir) = open_temp_repository().await;
    let existing = target
        .create_collection("Keep me".to_string())
        .await
        .unwrap();

    let result = target
        .import_workspace_snapshot(invalid_snapshot, false)
        .await;
    assert!(result.is_err());

    let after_failure = target.export_full_snapshot().await.unwrap();
    assert_eq!(after_failure.collections.len(), 1);
    assert_eq!(after_failure.collections[0].collection.id, existing.id);
    assert_eq!(after_failure.collections[0].collection.name, "Keep me");
}
