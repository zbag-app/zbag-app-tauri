#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use uuid::Uuid;
use zbag_core::sensitive::SensitiveString;

fn cleanup_db_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    for suffix in ["-journal", "-shm", "-wal"] {
        let mut file_name = path.as_os_str().to_os_string();
        file_name.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(file_name));
    }
}

pub struct DbFileCleanup {
    path: PathBuf,
}

impl DbFileCleanup {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for DbFileCleanup {
    fn drop(&mut self) {
        cleanup_db_files(&self.path);
    }
}

fn temp_db_path(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{prefix}_{}.sqlite", Uuid::new_v4()))
}

pub fn temp_db_path_with_cleanup(prefix: &str) -> (PathBuf, DbFileCleanup) {
    let path = temp_db_path(prefix);
    let cleanup = DbFileCleanup::new(path.clone());
    (path, cleanup)
}

pub fn solve_backup_challenge(
    seed_phrase: &[SensitiveString],
    indices: &[u8],
) -> HashMap<u8, SensitiveString> {
    indices
        .iter()
        .map(|idx| {
            let word_idx = (*idx as usize)
                .checked_sub(1)
                .unwrap_or_else(|| panic!("backup challenge index must be >= 1, got {idx}"));
            let word = seed_phrase
                .get(word_idx)
                .unwrap_or_else(|| {
                    panic!(
                        "backup challenge index {idx} out of range for seed phrase length {}",
                        seed_phrase.len()
                    )
                })
                .clone();
            (*idx, word)
        })
        .collect()
}
