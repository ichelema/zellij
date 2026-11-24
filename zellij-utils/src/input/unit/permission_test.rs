use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, UNIX_EPOCH};

fn cache_with(path: &PathBuf, other_permission: PermissionType) -> PermissionCache {
    let mut cache = PermissionCache::from_path_or_default(Some(path.clone()));
    cache.cache(
        "P".to_owned(),
        vec![
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ],
    );
    cache.cache("Q".to_owned(), vec![other_permission]);
    cache
}

#[test]
fn permission_cache_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("permissions.kdl");
    cache_with(&path, PermissionType::OpenFiles)
        .write_to_file()
        .unwrap();

    let read_back = PermissionCache::from_path_or_default(Some(path));
    assert!(read_back.check_permissions(
        "P".to_owned(),
        &vec![
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState
        ]
    ));
    assert!(read_back.check_permissions("Q".to_owned(), &vec![PermissionType::OpenFiles]));
    assert!(!read_back.check_permissions("Q".to_owned(), &vec![PermissionType::RunCommands]));
}

#[test]
fn permission_cache_unchanged_content_is_not_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("permissions.kdl");
    let mut cache = PermissionCache::from_path_or_default(Some(path.clone()));
    cache.cache("P".to_owned(), vec![PermissionType::ReadApplicationState]);
    cache.write_to_file().unwrap();

    let old_mtime = UNIX_EPOCH + Duration::from_secs(1000);
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(old_mtime)
        .unwrap();

    cache.write_to_file().unwrap();

    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old_mtime);
    let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 1, "no temporary file may be left behind");
}

#[test]
fn permission_cache_is_never_read_truncated_while_being_rewritten() {
    const WRITES: usize = 2000;
    const READERS: usize = 3;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("permissions.kdl");
    let caches = [
        cache_with(&path, PermissionType::OpenFiles),
        cache_with(&path, PermissionType::RunCommands),
    ];
    caches[0].write_to_file().unwrap();

    let writer_done = AtomicBool::new(false);
    let failed_reads = AtomicUsize::new(0);
    let total_reads = AtomicUsize::new(0);
    let required = vec![PermissionType::ReadApplicationState];

    std::thread::scope(|s| {
        for _ in 0..READERS {
            s.spawn(|| {
                while !writer_done.load(Ordering::Acquire) {
                    let cache = PermissionCache::from_path_or_default(Some(path.clone()));
                    total_reads.fetch_add(1, Ordering::Relaxed);
                    if !cache.check_permissions("P".to_owned(), &required) {
                        failed_reads.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
        for i in 0..WRITES {
            caches[i % 2].write_to_file().unwrap();
        }
        writer_done.store(true, Ordering::Release);
    });

    assert_eq!(
        failed_reads.load(Ordering::Relaxed),
        0,
        "readers saw a truncated permission cache ({} reads in total)",
        total_reads.load(Ordering::Relaxed)
    );
}
