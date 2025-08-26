use crate::error::{DocTreeError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub file_path: PathBuf,
    pub content_hash: String,
    pub summary: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheData {
    pub version: String,
    pub entries: HashMap<String, CacheEntry>,
}

impl Default for CacheData {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            entries: HashMap::new(),
        }
    }
}

pub struct CacheManager {
    cache_dir: PathBuf,
    cache_file: PathBuf,
    data: CacheData,
}

impl CacheManager {
    pub fn new(base_path: &Path, cache_dir_name: &str) -> Result<Self> {
        let cache_dir = base_path.join(cache_dir_name);
        let cache_file = cache_dir.join("cache.json");

        let mut manager = Self {
            cache_dir,
            cache_file,
            data: CacheData::default(),
        };

        manager.load_cache()?;
        Ok(manager)
    }

    pub fn initialize_cache_directory(&self) -> Result<()> {
        if !self.cache_dir.exists() {
            fs::create_dir_all(&self.cache_dir)
                .map_err(|e| DocTreeError::cache(format!("Failed to create cache directory: {e}")))?;
            log::info!("Created cache directory: {}", self.cache_dir.display());
        }

        // Update .gitignore to include cache directory
        self.update_gitignore()?;

        Ok(())
    }

    fn update_gitignore(&self) -> Result<()> {
        let cache_dir_name = self.cache_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| DocTreeError::cache("Invalid cache directory name"))?;

        let gitignore_path = self.cache_dir.parent()
            .ok_or_else(|| DocTreeError::cache("Invalid cache directory parent"))?
            .join(".gitignore");

        let gitignore_entry = format!("{cache_dir_name}/\n");

        if gitignore_path.exists() {
            let content = fs::read_to_string(&gitignore_path)?;
            if !content.contains(cache_dir_name) {
                fs::write(&gitignore_path, content + &gitignore_entry)?;
                log::info!("Added {cache_dir_name} to .gitignore");
            } else {
                log::debug!("Cache directory already in .gitignore");
            }
        } else {
            fs::write(&gitignore_path, gitignore_entry)?;
            log::info!("Created .gitignore with cache directory entry");
        }

        Ok(())
    }

    pub fn load_cache(&mut self) -> Result<()> {
        if self.cache_file.exists() {
            let content = fs::read_to_string(&self.cache_file)?;
            self.data = serde_json::from_str(&content)
                .map_err(|e| DocTreeError::cache(format!("Failed to parse cache: {e}")))?;
            
            log::info!("Loaded cache with {} entries", self.data.entries.len());
        } else {
            log::info!("No existing cache found, starting fresh");
            self.data = CacheData::default();
        }
        Ok(())
    }

    pub fn save_cache(&self) -> Result<()> {
        self.initialize_cache_directory()?;
        
        let content = serde_json::to_string_pretty(&self.data)
            .map_err(|e| DocTreeError::cache(format!("Failed to serialize cache: {e}")))?;
        
        fs::write(&self.cache_file, content)
            .map_err(|e| DocTreeError::cache(format!("Failed to write cache file: {e}")))?;
        
        log::debug!("Cache saved with {} entries", self.data.entries.len());
        Ok(())
    }

    pub fn get_cached_summary(&self, file_path: &Path, content_hash: &str) -> Option<String> {
        let key = self.path_to_cache_key(file_path);
        
        if let Some(entry) = self.data.entries.get(&key) {
            if entry.content_hash == content_hash {
                log::debug!("Cache hit for: {}", file_path.display());
                return Some(entry.summary.clone());
            } else {
                log::debug!("Cache miss (hash mismatch) for: {}", file_path.display());
            }
        } else {
            log::debug!("Cache miss (not found) for: {}", file_path.display());
        }
        
        None
    }

    pub fn store_summary(&mut self, file_path: &Path, content_hash: String, summary: String) -> Result<()> {
        let key = self.path_to_cache_key(file_path);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = CacheEntry {
            file_path: file_path.to_path_buf(),
            content_hash,
            summary,
            timestamp,
        };

        self.data.entries.insert(key, entry);
        log::debug!("Stored summary for: {}", file_path.display());
        
        Ok(())
    }

    pub fn invalidate_entry(&mut self, file_path: &Path) {
        let key = self.path_to_cache_key(file_path);
        if self.data.entries.remove(&key).is_some() {
            log::debug!("Invalidated cache entry for: {}", file_path.display());
        }
    }

    pub fn clear_cache(&mut self) -> Result<()> {
        self.data.entries.clear();
        
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)
                .map_err(|e| DocTreeError::cache(format!("Failed to remove cache directory: {e}")))?;
            log::info!("Cleared cache directory: {}", self.cache_dir.display());
        }
        
        Ok(())
    }

    pub fn get_cache_stats(&self) -> (usize, u64) {
        let entry_count = self.data.entries.len();
        let total_size = self.cache_file.metadata()
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        
        (entry_count, total_size)
    }

    fn path_to_cache_key(&self, path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    pub fn is_cache_valid(&self) -> bool {
        self.data.version == "1.0.0"
    }

    pub fn cleanup_old_entries(&mut self, max_age_days: u64) -> Result<()> {
        let cutoff_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() - (max_age_days * 24 * 60 * 60);

        let initial_count = self.data.entries.len();
        self.data.entries.retain(|_, entry| entry.timestamp > cutoff_time);
        let removed_count = initial_count - self.data.entries.len();

        if removed_count > 0 {
            log::info!("Cleaned up {removed_count} old cache entries");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_operations() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut cache = CacheManager::new(temp_dir.path(), ".test_cache")?;

        let test_path = PathBuf::from("test/file.rs");
        let hash = "testhash123".to_string();
        let summary = "Test summary".to_string();

        // Test storing and retrieving
        cache.store_summary(&test_path, hash.clone(), summary.clone())?;
        let retrieved = cache.get_cached_summary(&test_path, &hash);
        assert_eq!(retrieved, Some(summary));

        // Test cache miss with different hash
        let different_hash = "differenthash456";
        let retrieved_miss = cache.get_cached_summary(&test_path, different_hash);
        assert_eq!(retrieved_miss, None);

        // Test invalidation
        cache.invalidate_entry(&test_path);
        let after_invalidation = cache.get_cached_summary(&test_path, &hash);
        assert_eq!(after_invalidation, None);

        Ok(())
    }

    #[test]
    fn test_cache_persistence() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_path = PathBuf::from("test/file.rs");
        let hash = "testhash123".to_string();
        let summary = "Test summary".to_string();

        // Store in first instance
        {
            let mut cache1 = CacheManager::new(temp_dir.path(), ".test_cache")?;
            cache1.store_summary(&test_path, hash.clone(), summary.clone())?;
            cache1.save_cache()?;
        }

        // Load in second instance
        {
            let cache2 = CacheManager::new(temp_dir.path(), ".test_cache")?;
            let retrieved = cache2.get_cached_summary(&test_path, &hash);
            assert_eq!(retrieved, Some(summary));
        }

        Ok(())
    }
}