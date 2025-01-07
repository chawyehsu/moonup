use assert_fs::prelude::*;
use insta::assert_snapshot;
use moonup::archive::{extract_tar_gz, extract_zip};

use crate::util;

#[tokio::test]
async fn test_archive_zip_extraction() {
    util::apply_common_filters!();

    let tempdir = assert_fs::TempDir::new().unwrap();

    let zip = include_bytes!("../fixtures/archive/test.zip");

    let reader = tokio::io::BufReader::new(zip.as_ref());
    let hash = extract_zip(reader, tempdir.path())
        .await
        .expect("should extract zip");

    tempdir
        .child("hello.txt")
        .assert(predicates::path::exists());

    let sha256 = format!("{:x}", hash);
    assert_snapshot!(sha256, @"cf1b56aea8868e856e3345d4e8ed0fd2cd10907170a0bcbf4494ec532abb3e86");
}

#[tokio::test]
async fn test_archive_tar_gz_extraction() {
    util::apply_common_filters!();

    let tempdir = assert_fs::TempDir::new().unwrap();

    let tar_gz = include_bytes!("../fixtures/archive/test.tar.gz");

    let reader = tokio::io::BufReader::new(tar_gz.as_ref());
    let hash = extract_tar_gz(reader, tempdir.path())
        .await
        .expect("should extract tar.gz");

    tempdir
        .child("world.txt")
        .assert(predicates::path::exists());

    let sha256 = format!("{:x}", hash);
    assert_snapshot!(sha256, @"65f77ae8d172385a19157f338ca63f6cdb836e1fce82751c2ea8d7e5c7991823");
}
