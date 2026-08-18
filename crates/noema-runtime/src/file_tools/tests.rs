use super::*;

#[tokio::test]
async fn text_parser_bounds_utf8_without_using_anydoc() {
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("large.csv");
    std::fs::write(&path, "rank,domain\n1,éxample.com\n".repeat(200)).expect("write");
    let file = std::fs::File::open(&path).expect("open");
    let response = parse_open_file(file, "large.csv", Some("text/csv"), 1000).await;
    assert_eq!(response.status, FileParseStatus::Converted);
    assert_eq!(response.parser.as_deref(), Some("utf8"));
    assert_eq!(response.content_format.as_deref(), Some("csv"));
    assert_eq!(response.returned_chars, 1000);
    assert!(response.truncated);
}

#[test]
fn unsupported_document_has_a_bounded_result() {
    let response = convert_document_bytes(b"not a document", Some("bin"), 1000);
    assert_eq!(response.status, FileParseStatus::Unsupported);
    assert_eq!(response.error.as_deref(), Some("unsupported_format"));
    assert!(response.content.is_none());
    let response = convert_document_bytes(b"{\\rtf1\\ansi Parsed text}", Some("rtf"), 1000);
    assert_eq!(response.status, FileParseStatus::Converted);
    let content = response.content.as_deref().unwrap_or_default();
    assert!(content.contains("Parsed text"));
}

#[test]
fn download_paths_reject_traversal_and_never_replace_a_destination() {
    assert!(normalized_relative_path("../outside.csv").is_err());
    assert!(normalized_relative_path("/outside.csv").is_err());
    let directory = tempfile::tempdir().expect("temporary directory");
    std::fs::write(directory.path().join("target.csv"), "original").expect("target");
    std::fs::write(directory.path().join("temp.csv"), "replacement").expect("temporary file");
    let root = Dir::open_ambient_dir(directory.path(), ambient_authority()).expect("root");
    let error = commit_download(&root, Path::new("temp.csv"), Path::new("target.csv"))
        .expect_err("existing destination rejected");
    assert_eq!(error, "destination already exists");
    let content = std::fs::read_to_string(directory.path().join("target.csv")).unwrap();
    assert_eq!(content, "original");
    assert!(!directory.path().join("temp.csv").exists());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(".", directory.path().join("linked")).unwrap();
        assert!(prepare_download_parent(&root, Path::new("linked/file.csv")).is_err());
    }
}
