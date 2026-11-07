use std::path::PathBuf;

use lavunio_api::config::Config;
use lavunio_api::storage::{Storage, StorageKind};

#[tokio::test]
async fn s3_presign_generates_url_and_public_cdn_url() {
    let mut config = Config::local(String::new(), PathBuf::from("./data/uploads"));
    config.storage_kind = StorageKind::S3;
    config.s3_endpoint = Some("https://example.r2.cloudflarestorage.com".to_string());
    config.s3_bucket = Some("lavunio".to_string());
    config.s3_access_key_id = Some("test-key".to_string());
    config.s3_secret_access_key = Some("test-secret".to_string());
    config.s3_public_base_url = Some("https://cdn.example.com".to_string());
    config.s3_force_path_style = true;

    let storage = Storage::new(&config).expect("s3 storage");

    let url = storage
        .presign_put("modes/test/releases/1/build.zip", 3600)
        .await
        .expect("presigned url");
    assert!(url.contains("example.r2.cloudflarestorage.com"));
    assert!(url.contains("X-Amz-Signature"));

    assert_eq!(
        storage.public_url("modes/test/releases/1/build.zip"),
        "https://cdn.example.com/modes/test/releases/1/build.zip"
    );
}
