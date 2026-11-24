use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{consts::ZELLIJ_PLUGIN_PERMISSIONS_CACHE, data::PermissionType};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub type GrantedPermission = HashMap<String, Vec<PermissionType>>;

#[derive(Default, Debug)]
pub struct PermissionCache {
    path: PathBuf,
    granted: GrantedPermission,
}

impl PermissionCache {
    pub fn cache(&mut self, plugin_name: String, permissions: Vec<PermissionType>) {
        self.granted.insert(plugin_name, permissions);
    }

    pub fn get_permissions(&self, plugin_name: String) -> Option<&Vec<PermissionType>> {
        self.granted.get(&plugin_name)
    }

    pub fn check_permissions(
        &self,
        plugin_name: String,
        permissions_to_check: &Vec<PermissionType>,
    ) -> bool {
        if let Some(target) = self.granted.get(&plugin_name) {
            let mut all_granted = true;
            for permission in permissions_to_check {
                if !target.contains(permission) {
                    all_granted = false;
                }
            }
            return all_granted;
        }

        false
    }

    pub fn from_path_or_default(cache_path: Option<PathBuf>) -> Self {
        let cache_path = cache_path.unwrap_or(ZELLIJ_PLUGIN_PERMISSIONS_CACHE.to_path_buf());

        let granted = match fs::read_to_string(cache_path.clone()) {
            Ok(raw_string) => PermissionCache::from_string(raw_string).unwrap_or_default(),
            Err(e) => {
                log::error!("Failed to read permission cache file: {}", e);
                GrantedPermission::default()
            },
        };

        PermissionCache {
            path: cache_path,
            granted,
        }
    }

    pub fn write_to_file(&self) -> std::io::Result<()> {
        // Plugin threads read this file concurrently (`from_path_or_default`): truncating it
        // in place let them see an empty cache and ask for permissions that were already
        // granted. So skip identical content and otherwise replace the file atomically.
        let content = PermissionCache::to_string(&self.granted);
        if fs::read_to_string(&self.path).is_ok_and(|existing| existing == content) {
            return Ok(());
        }
        let mut temp_file_name = self.path.file_name().unwrap_or_default().to_os_string();
        temp_file_name.push(format!(
            ".{}.{}.tmp",
            std::process::id(),
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let temp_path = self.path.with_file_name(temp_file_name);
        let result = (|| {
            let mut f = File::create(&temp_path)?;
            write!(f, "{}", content)?;
            f.flush()?;
            fs::rename(&temp_path, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }
}

// The unit test location.
#[cfg(test)]
#[path = "./unit/permission_test.rs"]
mod permission_test;
