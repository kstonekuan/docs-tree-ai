use crate::error::{DocTreeError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSummary {
    pub source_path: PathBuf,
    pub content_hash: String,
    pub summary: String,
    pub timestamp: u64,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadmeLineMapping {
    pub line_number: usize,
    pub line_content: String,
    pub cache_keys: Vec<String>,
    pub last_validated_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadmeMappingData {
    pub version: String,
    pub readme_hash: String,
    pub mappings: Vec<ReadmeLineMapping>,
}

impl Default for ReadmeMappingData {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            readme_hash: String::new(),
            mappings: Vec::new(),
        }
    }
}


pub struct CacheManager {
    cache_dir: PathBuf,
    base_path: PathBuf,
    mapping_file: PathBuf,
    mapping_data: ReadmeMappingData,
}

impl CacheManager {
    pub fn new(base_path: &Path, cache_dir_name: &str) -> Result<Self> {
        let cache_dir = base_path.join(cache_dir_name);
        let mapping_file = cache_dir.join("readme_mapping.json");

        let mut manager = Self {
            cache_dir,
            base_path: base_path.to_path_buf(),
            mapping_file,
            mapping_data: ReadmeMappingData::default(),
        };

        manager.load_mapping()?;
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

    fn get_cache_path(&self, source_path: &Path) -> Result<PathBuf> {
        let relative_path = source_path.strip_prefix(&self.base_path)
            .unwrap_or(source_path);
        
        let cache_path = if source_path.is_dir() {
            self.cache_dir.join(relative_path).join(".dir_summary.json")
        } else {
            let mut cache_file = self.cache_dir.join(relative_path);
            let filename = format!("{}.summary.json", cache_file.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown"));
            cache_file.set_file_name(filename);
            cache_file
        };
        
        Ok(cache_path)
    }

    pub fn get_cached_summary(&self, source_path: &Path, content_hash: &str) -> Option<String> {
        let cache_path = self.get_cache_path(source_path).ok()?;
        
        if !cache_path.exists() {
            log::debug!("Cache miss (file not found) for: {}", source_path.display());
            return None;
        }
        
        let content = fs::read_to_string(&cache_path).ok()?;
        let cache_summary: CacheSummary = serde_json::from_str(&content).ok()?;
        
        if cache_summary.content_hash == content_hash {
            log::debug!("Cache hit for: {}", source_path.display());
            Some(cache_summary.summary)
        } else {
            log::debug!("Cache miss (hash mismatch) for: {}", source_path.display());
            None
        }
    }

    pub fn store_summary(&mut self, source_path: &Path, content_hash: String, summary: String) -> Result<()> {
        let cache_path = self.get_cache_path(source_path)?;
        
        // Create parent directory if needed
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| DocTreeError::cache(format!("Failed to create cache directory: {e}")))?;
        }
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let cache_summary = CacheSummary {
            source_path: source_path.to_path_buf(),
            content_hash,
            summary,
            timestamp,
            is_directory: source_path.is_dir(),
        };

        let content = serde_json::to_string_pretty(&cache_summary)
            .map_err(|e| DocTreeError::cache(format!("Failed to serialize cache: {e}")))?;
        
        fs::write(&cache_path, content)
            .map_err(|e| DocTreeError::cache(format!("Failed to write cache file: {e}")))?;
        
        log::debug!("Stored summary for: {} at {}", source_path.display(), cache_path.display());
        
        Ok(())
    }

    pub fn invalidate_entry(&mut self, source_path: &Path) -> Result<()> {
        let cache_path = self.get_cache_path(source_path)?;
        
        if cache_path.exists() {
            fs::remove_file(&cache_path)
                .map_err(|e| DocTreeError::cache(format!("Failed to remove cache file: {e}")))?;
            log::debug!("Invalidated cache entry for: {}", source_path.display());
        }
        
        Ok(())
    }

    pub fn clear_cache(&mut self) -> Result<()> {
        if self.cache_dir.exists() {
            // Remove all .summary.json and .dir_summary.json files but keep mappings
            Self::clear_cache_files(&self.cache_dir)?;
            log::info!("Cleared cache files in: {}", self.cache_dir.display());
        }
        
        Ok(())
    }
    
    fn clear_cache_files(dir: &Path) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                Self::clear_cache_files(&path)?;
                // Remove empty directories
                if fs::read_dir(&path)?.next().is_none() {
                    fs::remove_dir(&path)?;
                }
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".summary.json") || name == ".dir_summary.json" {
                    fs::remove_file(&path)?;
                }
            }
        }
        Ok(())
    }

    pub fn get_cache_stats(&self) -> (usize, u64) {
        let mut entry_count = 0;
        let mut total_size = 0u64;
        
        if self.cache_dir.exists() {
            Self::count_cache_files(&self.cache_dir, &mut entry_count, &mut total_size);
        }
        
        (entry_count, total_size)
    }
    
    fn count_cache_files(dir: &Path, count: &mut usize, size: &mut u64) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                
                if path.is_dir() {
                    Self::count_cache_files(&path, count, size);
                } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".summary.json") || name == ".dir_summary.json" {
                        *count += 1;
                        if let Ok(metadata) = path.metadata() {
                            *size += metadata.len();
                        }
                    }
                }
            }
        }
    }


    pub fn is_cache_valid(&self) -> bool {
        // Cache is always valid in the new structure since each file is independent
        true
    }

    pub fn cleanup_old_entries(&mut self, max_age_days: u64) -> Result<()> {
        let cutoff_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() - (max_age_days * 24 * 60 * 60);

        Self::cleanup_old_files(&self.cache_dir, cutoff_time)?;
        Ok(())
    }
    
    fn cleanup_old_files(dir: &Path, cutoff_time: u64) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                Self::cleanup_old_files(&path, cutoff_time)?;
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".summary.json") || name == ".dir_summary.json" {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(summary) = serde_json::from_str::<CacheSummary>(&content) {
                            if summary.timestamp < cutoff_time {
                                fs::remove_file(&path)?;
                                log::debug!("Removed old cache file: {}", path.display());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn load_mapping(&mut self) -> Result<()> {
        if self.mapping_file.exists() {
            let content = fs::read_to_string(&self.mapping_file)?;
            self.mapping_data = serde_json::from_str(&content)
                .map_err(|e| DocTreeError::cache(format!("Failed to parse mapping: {e}")))?;
            
            log::info!("Loaded README mapping with {} entries", self.mapping_data.mappings.len());
        } else {
            log::info!("No existing README mapping found");
            self.mapping_data = ReadmeMappingData::default();
        }
        Ok(())
    }

    pub fn save_mapping(&self) -> Result<()> {
        self.initialize_cache_directory()?;
        
        let content = serde_json::to_string_pretty(&self.mapping_data)
            .map_err(|e| DocTreeError::cache(format!("Failed to serialize mapping: {e}")))?;
        
        fs::write(&self.mapping_file, content)
            .map_err(|e| DocTreeError::cache(format!("Failed to write mapping file: {e}")))?;
        
        log::debug!("README mapping saved with {} entries", self.mapping_data.mappings.len());
        Ok(())
    }


    pub fn update_readme_mapping(&mut self, readme_hash: String, mappings: Vec<ReadmeLineMapping>) -> Result<()> {
        self.mapping_data.readme_hash = readme_hash;
        self.mapping_data.mappings = mappings;
        self.save_mapping()
    }

    pub fn get_readme_mapping(&self) -> &ReadmeMappingData {
        &self.mapping_data
    }

    pub fn get_affected_readme_lines(&self, cache_key: &str) -> Vec<usize> {
        self.mapping_data.mappings
            .iter()
            .filter(|mapping| mapping.cache_keys.contains(&cache_key.to_string()))
            .map(|mapping| mapping.line_number)
            .collect()
    }

    pub fn validate_readme_hash(&self, current_hash: &str) -> bool {
        self.mapping_data.readme_hash == current_hash
    }

    pub fn get_cache_summary(&self, source_path: &Path) -> Option<CacheSummary> {
        let cache_path = self.get_cache_path(source_path).ok()?;
        
        if !cache_path.exists() {
            return None;
        }
        
        let content = fs::read_to_string(&cache_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn get_all_summaries(&self) -> Vec<CacheSummary> {
        let mut summaries = Vec::new();
        if self.cache_dir.exists() {
            Self::collect_summaries(&self.cache_dir, &mut summaries);
        }
        summaries
    }
    
    fn collect_summaries(dir: &Path, summaries: &mut Vec<CacheSummary>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                
                if path.is_dir() {
                    Self::collect_summaries(&path, summaries);
                } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".summary.json") || name == ".dir_summary.json" {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(summary) = serde_json::from_str::<CacheSummary>(&content) {
                                summaries.push(summary);
                            }
                        }
                    }
                }
            }
        }
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
        cache.invalidate_entry(&test_path)?;
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
            // Cache is automatically persisted when store_summary is called
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