//! Finalized model files must survive unsuccessful imports and self-imports.

use holzi_lib::models::import::copy_into_managed;

#[tokio::test]
async fn reimporting_the_managed_file_preserves_its_contents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let model = dir.path().join("model.gguf");
    let contents = b"GGUF existing model bytes";
    tokio::fs::write(&model, contents)
        .await
        .expect("seed model");

    let copied = copy_into_managed(&model, model.clone())
        .await
        .expect("reimport managed file");

    assert_eq!(copied, contents.len() as u64);
    assert_eq!(tokio::fs::read(&model).await.expect("read model"), contents);
}

#[tokio::test]
async fn successful_import_replaces_only_the_destination() {
    let dir = tempfile::tempdir().expect("temp dir");
    let source = dir.path().join("source.gguf");
    let destination = dir.path().join("managed.gguf");
    let contents = b"GGUF replacement model";
    tokio::fs::write(&source, contents).await.expect("source");
    tokio::fs::write(&destination, b"old model")
        .await
        .expect("destination");

    assert_eq!(
        copy_into_managed(&source, destination.clone())
            .await
            .expect("import"),
        contents.len() as u64,
    );
    assert_eq!(
        tokio::fs::read(&destination)
            .await
            .expect("read destination"),
        contents
    );
    assert_eq!(
        tokio::fs::read(&source).await.expect("read source"),
        contents
    );
}

#[tokio::test]
async fn failed_publication_preserves_destination_and_removes_staging() {
    let dir = tempfile::tempdir().expect("temp dir");
    let source = dir.path().join("source.gguf");
    let destination = dir.path().join("managed.gguf");
    tokio::fs::write(&source, b"GGUF replacement")
        .await
        .expect("source");
    tokio::fs::create_dir(&destination)
        .await
        .expect("destination directory");
    let existing = destination.join("keep");
    tokio::fs::write(&existing, b"existing")
        .await
        .expect("existing file");

    assert!(copy_into_managed(&source, destination).await.is_err());
    assert_eq!(
        tokio::fs::read(existing).await.expect("read existing"),
        b"existing"
    );
    let mut entries = tokio::fs::read_dir(dir.path()).await.expect("read dir");
    while let Some(entry) = entries.next_entry().await.expect("entry") {
        assert_ne!(
            entry.path().extension().and_then(|e| e.to_str()),
            Some("tmp")
        );
    }
}
