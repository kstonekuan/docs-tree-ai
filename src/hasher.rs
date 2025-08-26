use crate::error::{DocTreeError, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub struct FileHasher;

impl FileHasher {
    pub fn compute_file_hash(file_path: &Path) -> Result<String> {
        log::debug!("Computing hash for file: {}", file_path.display());
        
        let file = File::open(file_path)
            .map_err(DocTreeError::Io)?;
        
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let hash = hasher.finalize();
        let hash_string = format!("{hash:x}");
        
        log::debug!("Hash computed: {} -> {}", file_path.display(), &hash_string[..8]);
        
        Ok(hash_string)
    }

    pub fn compute_content_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = hasher.finalize();
        format!("{hash:x}")
    }

    pub fn compute_directory_hash(children_hashes: &[String]) -> String {
        let combined = children_hashes.join("|");
        Self::compute_content_hash(&combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_compute_content_hash() {
        let content = "Hello, World!";
        let hash1 = FileHasher::compute_content_hash(content);
        let hash2 = FileHasher::compute_content_hash(content);
        
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 produces 64-character hex string
    }

    #[test]
    fn test_different_content_different_hash() {
        let content1 = "Hello, World!";
        let content2 = "Goodbye, World!";
        
        let hash1 = FileHasher::compute_content_hash(content1);
        let hash2 = FileHasher::compute_content_hash(content2);
        
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_file_hash() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "Test content for hashing")?;
        
        let hash = FileHasher::compute_file_hash(temp_file.path())?;
        assert_eq!(hash.len(), 64);
        
        Ok(())
    }

    #[test]
    fn test_compute_directory_hash() {
        let children_hashes = vec![
            "hash1".to_string(),
            "hash2".to_string(),
            "hash3".to_string(),
        ];
        
        let dir_hash = FileHasher::compute_directory_hash(&children_hashes);
        assert_eq!(dir_hash.len(), 64);
        
        // Same children should produce same hash
        let dir_hash2 = FileHasher::compute_directory_hash(&children_hashes);
        assert_eq!(dir_hash, dir_hash2);
        
        // Different order should produce different hash
        let mut different_order = children_hashes.clone();
        different_order.reverse();
        let dir_hash3 = FileHasher::compute_directory_hash(&different_order);
        assert_ne!(dir_hash, dir_hash3);
    }
}