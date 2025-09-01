use crate::cache::CacheManager;
use crate::error::{DocTreeError, Result};
use crate::hasher::FileHasher;
use crate::llm::LanguageModelClient;
use crate::scanner::{DirectoryScanner, FileNode};
use std::fs;
use std::path::Path;

pub struct HierarchicalSummarizer {
    llm_client: LanguageModelClient,
    cache_manager: CacheManager,
    force_regeneration: bool,
}

impl HierarchicalSummarizer {
    pub fn new(
        llm_client: LanguageModelClient,
        cache_manager: CacheManager,
        force_regeneration: bool,
    ) -> Self {
        Self {
            llm_client,
            cache_manager,
            force_regeneration,
        }
    }

    pub async fn generate_project_summary(&mut self, base_path: &Path) -> Result<String> {
        log::info!("Starting hierarchical summarization for: {}", base_path.display());

        // Initialize cache directory
        self.cache_manager.initialize_cache_directory()?;

        // Scan directory structure
        let scanner = DirectoryScanner::new(base_path.to_path_buf());
        let mut root_node = scanner.scan_directory()?;

        // Generate summaries in bottom-up fashion (post-order traversal)
        self.summarize_tree(&mut root_node, base_path).await?;

        // Cache is saved incrementally during processing

        // Return root-level summary
        root_node.summary.ok_or_else(|| {
            DocTreeError::summarizer("Failed to generate root-level project summary")
        })
    }

    fn summarize_tree<'a>(
        &'a mut self,
        node: &'a mut FileNode,
        base_path: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + 'a>> {
        Box::pin(async move {
            if node.is_directory {
                // First, recursively process all children
                for child in &mut node.children {
                    self.summarize_tree(child, base_path).await?;
                }

                // Then generate summary for this directory
                self.summarize_directory(node, base_path).await
            } else {
                // Generate summary for file
                self.summarize_file(node, base_path).await
            }
        })
    }

    async fn summarize_file(&mut self, node: &mut FileNode, base_path: &Path) -> Result<()> {
        if !node.is_source_code_file() {
            log::debug!("Skipping non-source file: {}", node.path.display());
            return Ok(());
        }

        log::debug!("Processing file: {}", node.path.display());

        // Compute file hash
        let content_hash = FileHasher::compute_file_hash(&node.path)?;
        node.content_hash = Some(content_hash.clone());

        // Check cache first (unless force regeneration is enabled)
        if !self.force_regeneration {
            if let Some(cached_summary) = self.cache_manager.get_cached_summary(&node.path, &content_hash) {
                node.summary = Some(cached_summary);
                return Ok(());
            }
        }

        // Read file content
        let content = match fs::read_to_string(&node.path) {
            Ok(content) => {
                if content.trim().is_empty() {
                    log::debug!("Skipping empty file: {}", node.path.display());
                    return Ok(());
                }
                content
            }
            Err(e) => {
                log::warn!("Failed to read file {}: {}", node.path.display(), e);
                return Ok(());
            }
        };

        // Generate summary using LLM
        let relative_path = node.get_relative_path(base_path)?;
        match self.llm_client.generate_file_summary(&relative_path, &content).await {
            Ok(summary) => {
                node.summary = Some(summary.clone());
                // Store in cache
                self.cache_manager.store_summary(&node.path, content_hash, summary)?;
                log::info!("Generated summary for: {}", relative_path.display());
            }
            Err(e) => {
                log::error!("Failed to generate summary for {}: {}", relative_path.display(), e);
                // Continue processing other files even if one fails
            }
        }

        Ok(())
    }

    async fn summarize_directory(&mut self, node: &mut FileNode, base_path: &Path) -> Result<()> {
        let relative_path = node.get_relative_path(base_path)?;
        log::debug!("Processing directory: {}", relative_path.display());

        // Collect summaries from children
        let mut children_summaries = Vec::new();
        
        for child in &node.children {
            if let Some(ref summary) = child.summary {
                let child_relative_path = child.get_relative_path(base_path)?;
                let child_name = child_relative_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                
                let formatted_summary = if child.is_directory {
                    format!("**{child_name}/** (directory): {summary}")
                } else {
                    format!("**{child_name}**: {summary}")
                };
                
                children_summaries.push(formatted_summary);
            }
        }

        if children_summaries.is_empty() {
            log::debug!("No summarizable content in directory: {}", relative_path.display());
            return Ok(());
        }

        // Compute directory hash based on children hashes
        let children_hashes: Vec<String> = node.children
            .iter()
            .filter_map(|child| child.content_hash.clone())
            .collect();
        
        let directory_hash = FileHasher::compute_directory_hash(&children_hashes);
        node.content_hash = Some(directory_hash.clone());

        // Check cache for directory summary
        if !self.force_regeneration {
            if let Some(cached_summary) = self.cache_manager.get_cached_summary(&node.path, &directory_hash) {
                node.summary = Some(cached_summary);
                return Ok(());
            }
        }

        // Generate directory summary using LLM
        let directory_name = relative_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project root");

        match self.llm_client.generate_directory_summary(directory_name, &children_summaries).await {
            Ok(summary) => {
                node.summary = Some(summary.clone());
                // Store in cache
                self.cache_manager.store_summary(&node.path, directory_hash, summary)?;
                log::info!("Generated directory summary for: {}", relative_path.display());
            }
            Err(e) => {
                log::error!("Failed to generate directory summary for {}: {}", relative_path.display(), e);
                // Fall back to concatenating children summaries
                let fallback_summary = format!("Contains: {}", children_summaries.join(", "));
                node.summary = Some(fallback_summary);
            }
        }

        Ok(())
    }

    pub fn get_cache_stats(&self) -> (usize, u64) {
        self.cache_manager.get_cache_stats()
    }

    pub async fn cleanup_cache(&mut self, max_age_days: u64) -> Result<()> {
        self.cache_manager.cleanup_old_entries(max_age_days)
    }

    pub fn print_tree_summary(node: &FileNode, base_path: &Path, indent: usize) {
        let relative_path = node.get_relative_path(base_path).unwrap_or_else(|_| node.path.clone());
        let indent_str = "  ".repeat(indent);
        
        if node.is_directory {
            println!("{}📁 {}/", indent_str, relative_path.display());
        } else {
            println!("{}📄 {}", indent_str, relative_path.display());
        }

        if let Some(ref summary) = node.summary {
            let summary_preview = if summary.len() > 100 {
                format!("{}...", &summary[..97])
            } else {
                summary.clone()
            };
            println!("{indent_str}   → {summary_preview}");
        }

        for child in &node.children {
            Self::print_tree_summary(child, base_path, indent + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::TempDir;

    async fn create_test_summarizer() -> (HierarchicalSummarizer, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        
        let config = Config {
            openai_api_base: "http://localhost:11434/v1".to_string(),
            openai_api_key: "test".to_string(),
            openai_model_name: "test-model".to_string(),
            cache_dir_name: ".test_cache".to_string(),
            log_level: "debug".to_string(),
        };

        let llm_client = LanguageModelClient::new(&config).unwrap();
        let cache_manager = CacheManager::new(temp_dir.path(), ".test_cache").unwrap();
        
        let summarizer = HierarchicalSummarizer::new(llm_client, cache_manager, false);
        
        (summarizer, temp_dir)
    }

    #[tokio::test]
    async fn test_summarizer_creation() {
        let (summarizer, _temp_dir) = create_test_summarizer().await;
        assert!(!summarizer.force_regeneration);
    }

    #[test]
    fn test_file_node_operations() {
        let mut parent = FileNode::new("/tmp/test".into(), true);
        let child = FileNode::new("/tmp/test/file.rs".into(), false);
        
        parent.add_child(child);
        assert_eq!(parent.children.len(), 1);
        
        let source_file = FileNode::new("test.rs".into(), false);
        assert!(source_file.is_source_code_file());
        
        let non_source_file = FileNode::new("test.txt".into(), false);
        assert!(!non_source_file.is_source_code_file());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let (summarizer, _temp_dir) = create_test_summarizer().await;
        let (count, _size) = summarizer.get_cache_stats();
        assert_eq!(count, 0); // Empty cache initially
    }
}