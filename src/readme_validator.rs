use crate::cache::{CacheManager, ReadmeLineMapping};
use crate::error::{DocTreeError, Result};
use crate::hasher::FileHasher;
use crate::llm::LanguageModelClient;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub line_number: usize,
    pub current_content: String,
    pub suggested_content: String,
    pub reason: String,
    pub affected_cache_entries: Vec<String>,
}

pub struct ReadmeValidator {
    cache_manager: CacheManager,
    llm_client: LanguageModelClient,
}

impl ReadmeValidator {
    pub fn new(cache_manager: CacheManager, llm_client: LanguageModelClient) -> Self {
        Self {
            cache_manager,
            llm_client,
        }
    }

    pub async fn validate_readme(
        &mut self,
        base_path: &Path,
        project_summary: &str,
    ) -> Result<Vec<ValidationResult>> {
        let readme_path = base_path.join("README.md");

        if !readme_path.exists() {
            return Ok(vec![ValidationResult {
                line_number: 0,
                current_content: String::new(),
                suggested_content: format!(
                    "# {}\n\n{}",
                    base_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Project"),
                    project_summary
                ),
                reason: "README.md does not exist".to_string(),
                affected_cache_entries: vec![],
            }]);
        }

        let readme_content = fs::read_to_string(&readme_path)
            .map_err(|e| DocTreeError::readme(format!("Failed to read README.md: {e}")))?;

        let readme_hash = FileHasher::compute_content_hash(&readme_content);

        if !self.cache_manager.validate_readme_hash(&readme_hash) {
            log::info!("README has changed, regenerating mappings");
            let new_mappings = self.generate_mappings(&readme_content, base_path).await?;
            self.cache_manager
                .update_readme_mapping(readme_hash.clone(), new_mappings)?;
        }

        let mut validation_results = Vec::new();

        let mappings = &self.cache_manager.get_readme_mapping().mappings;

        for mapping in mappings {
            let validation_needed = mapping.cache_keys.iter().any(|key| {
                // Parse key as path to get cache summary
                let source_path = Path::new(key);
                if let Some(summary) = self.cache_manager.get_cache_summary(source_path) {
                    mapping.last_validated_hash.as_ref() != Some(&summary.content_hash)
                } else {
                    true
                }
            });

            if validation_needed {
                if let Some(suggestion) = self.suggest_update(mapping, project_summary).await? {
                    validation_results.push(suggestion);
                }
            }
        }

        Ok(validation_results)
    }

    async fn generate_mappings(
        &self,
        readme_content: &str,
        base_path: &Path,
    ) -> Result<Vec<ReadmeLineMapping>> {
        let mut mappings = Vec::new();

        for (line_number, line) in readme_content.lines().enumerate() {
            let line_number = line_number + 1;

            if self.is_content_line(line) {
                let cache_keys = self.find_relevant_cache_keys(line, base_path)?;

                if !cache_keys.is_empty() {
                    mappings.push(ReadmeLineMapping {
                        line_number,
                        line_content: line.to_string(),
                        cache_keys,
                        last_validated_hash: None,
                    });
                }
            }
        }

        Ok(mappings)
    }

    fn is_content_line(&self, line: &str) -> bool {
        let trimmed = line.trim();

        !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("```")
            && !trimmed.starts_with("---")
            && !trimmed.starts_with("***")
            && !trimmed.starts_with("___")
            && (trimmed.contains("module")
                || trimmed.contains("function")
                || trimmed.contains("class")
                || trimmed.contains("component")
                || trimmed.contains("file")
                || trimmed.contains("directory")
                || trimmed.contains("API")
                || trimmed.contains("endpoint")
                || trimmed.contains("service")
                || trimmed.contains("manager")
                || trimmed.contains("handler")
                || trimmed.contains("validator")
                || trimmed.contains("scanner")
                || trimmed.contains("client")
                || trimmed.contains("cache")
                || trimmed.contains("config")
                || trimmed.contains("error")
                || trimmed.contains("test")
                || trimmed.contains("util")
                || trimmed.contains("lib")
                || trimmed.contains("src/")
                || trimmed.contains(".rs")
                || trimmed.contains(".py")
                || trimmed.contains(".js")
                || trimmed.contains(".ts")
                || trimmed.contains(".go")
                || trimmed.contains(".java")
                || trimmed.contains(".cpp")
                || trimmed.contains(".c")
                || trimmed.contains(".h"))
    }

    fn find_relevant_cache_keys(&self, line: &str, base_path: &Path) -> Result<Vec<String>> {
        let mut cache_keys = Vec::new();
        let line_lower = line.to_lowercase();

        for summary in self.cache_manager.get_all_summaries() {
            let relative_path = summary
                .source_path
                .strip_prefix(base_path)
                .unwrap_or(&summary.source_path);

            let path_str = relative_path.to_string_lossy().to_lowercase();
            let file_name = relative_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();

            let file_stem = relative_path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();

            let key = summary.source_path.to_string_lossy().to_string();

            if (line_lower.contains(&path_str)
                || line_lower.contains(&file_name)
                || (file_stem.len() > 3 && line_lower.contains(&file_stem)))
                && !cache_keys.contains(&key)
            {
                cache_keys.push(key.clone());
            }

            let summary_keywords: Vec<&str> = summary
                .summary
                .split_whitespace()
                .filter(|w| w.len() > 5)
                .take(5)
                .collect();

            let matching_keywords = summary_keywords
                .iter()
                .filter(|keyword| line_lower.contains(&keyword.to_lowercase()))
                .count();

            if matching_keywords >= 2 && !cache_keys.contains(&key) {
                cache_keys.push(key);
            }
        }

        Ok(cache_keys)
    }

    async fn suggest_update(
        &self,
        mapping: &ReadmeLineMapping,
        project_summary: &str,
    ) -> Result<Option<ValidationResult>> {
        let mut relevant_summaries = Vec::new();

        for key in &mapping.cache_keys {
            let source_path = Path::new(key);
            if let Some(summary) = self.cache_manager.get_cache_summary(source_path) {
                let relative_path = summary
                    .source_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                relevant_summaries.push(format!("{}: {}", relative_path, summary.summary));
            }
        }

        if relevant_summaries.is_empty() {
            return Ok(None);
        }

        let combined_summaries = relevant_summaries.join("\n");

        let prompt = format!(
            "The following line in README.md may be outdated:\n\n\
            Line {}: \"{}\"\n\n\
            Current code summaries:\n{}\n\n\
            Project context:\n{}\n\n\
            If this line needs updating based on the current code, provide a corrected version. \
            If the line is still accurate, respond with 'NO_CHANGE'. \
            Only provide the updated line text, nothing else.",
            mapping.line_number, mapping.line_content, combined_summaries, project_summary
        );

        let response = self.llm_client.generate_readme_suggestion(&prompt).await?;

        if response.trim() != "NO_CHANGE" && response.trim() != mapping.line_content {
            Ok(Some(ValidationResult {
                line_number: mapping.line_number,
                current_content: mapping.line_content.clone(),
                suggested_content: response.trim().to_string(),
                reason: "Content outdated based on current code".to_string(),
                affected_cache_entries: mapping.cache_keys.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn print_validation_results(results: &[ValidationResult]) {
        if results.is_empty() {
            println!("✅ README.md is up-to-date with the current codebase");
            return;
        }

        println!("📋 README.md Validation Results");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        for result in results {
            println!("\n⚠️  Line {}: {}", result.line_number, result.reason);
            println!("   Current: \"{}\"", result.current_content);
            println!("   Suggested: \"{}\"", result.suggested_content);

            if !result.affected_cache_entries.is_empty() {
                println!("   Affected files:");
                for entry in &result.affected_cache_entries {
                    println!("     - {}", entry);
                }
            }
        }

        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("💡 {} lines need updating", results.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::TempDir;

    fn create_test_validator() -> (ReadmeValidator, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            openai_api_base: "http://localhost:11434/v1".to_string(),
            openai_api_key: "test".to_string(),
            openai_model_name: "test-model".to_string(),
            cache_dir_name: ".test_cache".to_string(),
            log_level: "debug".to_string(),
        };

        let cache_manager = CacheManager::new(temp_dir.path(), ".test_cache").unwrap();
        let llm_client = LanguageModelClient::new(&config).unwrap();

        let validator = ReadmeValidator::new(cache_manager, llm_client);
        (validator, temp_dir)
    }

    #[test]
    fn test_is_content_line() {
        let (validator, _) = create_test_validator();

        assert!(validator.is_content_line("The cache module handles persistence"));
        assert!(validator.is_content_line("Located in src/cache.rs"));
        assert!(validator.is_content_line("The Scanner class provides directory traversal"));

        assert!(!validator.is_content_line("# Header"));
        assert!(!validator.is_content_line(""));
        assert!(!validator.is_content_line("```rust"));
        assert!(!validator.is_content_line("---"));
    }

    #[test]
    fn test_validation_result_display() {
        let results = vec![ValidationResult {
            line_number: 42,
            current_content: "Old content".to_string(),
            suggested_content: "New content".to_string(),
            reason: "Outdated".to_string(),
            affected_cache_entries: vec!["src/main.rs".to_string()],
        }];

        ReadmeValidator::print_validation_results(&results);
    }
}
