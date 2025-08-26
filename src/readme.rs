use crate::error::{DocTreeError, Result};
use crate::llm::LanguageModelClient;
use std::fs;
use std::path::Path;

pub struct ReadmeManager {
    llm_client: LanguageModelClient,
}

impl ReadmeManager {
    pub fn new(llm_client: LanguageModelClient) -> Self {
        Self { llm_client }
    }

    pub async fn update_readme(&self, base_path: &Path, project_summary: &str) -> Result<()> {
        let readme_path = base_path.join("README.md");
        
        if readme_path.exists() {
            log::info!("Updating existing README.md");
            self.update_existing_readme(&readme_path, project_summary).await
        } else {
            log::info!("Creating new README.md");
            self.create_new_readme(&readme_path, project_summary, base_path).await
        }
    }

    async fn update_existing_readme(&self, readme_path: &Path, project_summary: &str) -> Result<()> {
        // Read existing README content
        let existing_content = fs::read_to_string(readme_path)
            .map_err(|e| DocTreeError::readme(format!("Failed to read README.md: {e}")))?;

        // Use LLM to intelligently merge the new summary with existing content
        let updated_content = self.llm_client
            .update_readme(&existing_content, project_summary)
            .await?;

        // Write updated content back
        fs::write(readme_path, updated_content)
            .map_err(|e| DocTreeError::readme(format!("Failed to write README.md: {e}")))?;

        log::info!("Successfully updated README.md");
        Ok(())
    }

    async fn create_new_readme(&self, readme_path: &Path, project_summary: &str, base_path: &Path) -> Result<()> {
        // Derive project name from directory name
        let project_name = base_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Project");

        // Generate new README content using LLM
        let readme_content = self.llm_client
            .create_new_readme(project_summary, project_name)
            .await?;

        // Write new README file
        fs::write(readme_path, readme_content)
            .map_err(|e| DocTreeError::readme(format!("Failed to create README.md: {e}")))?;

        log::info!("Successfully created new README.md");
        Ok(())
    }

    pub fn readme_exists(&self, base_path: &Path) -> bool {
        base_path.join("README.md").exists()
    }

    pub fn get_readme_info(&self, base_path: &Path) -> Result<ReadmeInfo> {
        let readme_path = base_path.join("README.md");
        
        if !readme_path.exists() {
            return Ok(ReadmeInfo {
                exists: false,
                size: 0,
                has_project_description: false,
                sections: Vec::new(),
            });
        }

        let content = fs::read_to_string(&readme_path)
            .map_err(|e| DocTreeError::readme(format!("Failed to read README.md: {e}")))?;

        let size = content.len();
        let has_project_description = self.detect_project_description(&content);
        let sections = self.extract_sections(&content);

        Ok(ReadmeInfo {
            exists: true,
            size,
            has_project_description,
            sections,
        })
    }

    fn detect_project_description(&self, content: &str) -> bool {
        let content_lower = content.to_lowercase();
        
        // Look for common indicators of project description
        content_lower.contains("description") ||
        content_lower.contains("about") ||
        content_lower.contains("overview") ||
        content_lower.contains("what is") ||
        content_lower.contains("purpose")
    }

    fn extract_sections(&self, content: &str) -> Vec<String> {
        let mut sections = Vec::new();
        
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Detect markdown headers (# ## ### etc.)
            if trimmed.starts_with('#') && trimmed.len() > 1 {
                // Extract section title after the hash marks
                let title = trimmed.trim_start_matches('#').trim().to_string();
                if !title.is_empty() {
                    sections.push(title);
                }
            }
        }
        
        sections
    }

    pub async fn create_minimal_readme(&self, base_path: &Path, project_summary: &str) -> Result<()> {
        let readme_path = base_path.join("README.md");
        
        let project_name = base_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Project");

        let minimal_content = format!(
            "# {project_name}\n\n{project_summary}\n\n## Installation\n\nTODO: Add installation instructions\n\n## Usage\n\nTODO: Add usage examples\n\n## Contributing\n\nTODO: Add contribution guidelines\n\n## License\n\nTODO: Add license information\n"
        );

        fs::write(&readme_path, minimal_content)
            .map_err(|e| DocTreeError::readme(format!("Failed to create minimal README.md: {e}")))?;

        log::info!("Created minimal README.md template");
        Ok(())
    }
}

#[derive(Debug)]
pub struct ReadmeInfo {
    pub exists: bool,
    pub size: usize,
    pub has_project_description: bool,
    pub sections: Vec<String>,
}

impl ReadmeInfo {
    pub fn print_summary(&self) {
        if self.exists {
            println!("README.md exists ({} bytes)", self.size);
            println!("Has project description: {}", self.has_project_description);
            
            if !self.sections.is_empty() {
                println!("Sections found:");
                for (i, section) in self.sections.iter().enumerate() {
                    println!("  {}. {}", i + 1, section);
                }
            } else {
                println!("No sections detected");
            }
        } else {
            println!("README.md does not exist");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_manager() -> ReadmeManager {
        let config = Config {
            openai_api_base: "http://localhost:11434/v1".to_string(),
            openai_api_key: "test".to_string(),
            openai_model_name: "test-model".to_string(),
            cache_dir_name: ".test_cache".to_string(),
            log_level: "debug".to_string(),
        };

        let llm_client = LanguageModelClient::new(&config).unwrap();
        ReadmeManager::new(llm_client)
    }

    #[test]
    fn test_readme_exists() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = create_test_manager();
        
        // Initially should not exist
        assert!(!manager.readme_exists(temp_dir.path()));
        
        // Create a README file
        let readme_path = temp_dir.path().join("README.md");
        fs::write(&readme_path, "# Test Project\n\nThis is a test.")?;
        
        // Now should exist
        assert!(manager.readme_exists(temp_dir.path()));
        
        Ok(())
    }

    #[test]
    fn test_extract_sections() {
        let manager = create_test_manager();
        
        let content = "# Main Title\n\n## Installation\n\nSome content\n\n### Subsection\n\n## Usage\n\nMore content";
        let sections = manager.extract_sections(content);
        
        assert_eq!(sections, vec!["Main Title", "Installation", "Subsection", "Usage"]);
    }

    #[test]
    fn test_detect_project_description() {
        let manager = create_test_manager();
        
        let content_with_desc = "This project is about creating awesome software";
        assert!(manager.detect_project_description(content_with_desc));
        
        let content_without_desc = "# Installation\n\nRun `npm install`";
        assert!(!manager.detect_project_description(content_without_desc));
    }

    #[test]
    fn test_get_readme_info() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = create_test_manager();
        
        // Test non-existent README
        let info = manager.get_readme_info(temp_dir.path())?;
        assert!(!info.exists);
        assert_eq!(info.size, 0);
        assert!(!info.has_project_description);
        assert!(info.sections.is_empty());
        
        // Create a README with content
        let readme_content = "# Test Project\n\n## About\n\nThis project does amazing things.\n\n## Installation\n\nRun the installer.";
        let readme_path = temp_dir.path().join("README.md");
        fs::write(&readme_path, readme_content)?;
        
        let info = manager.get_readme_info(temp_dir.path())?;
        assert!(info.exists);
        assert_eq!(info.size, readme_content.len());
        assert!(info.has_project_description);
        assert_eq!(info.sections, vec!["Test Project", "About", "Installation"]);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_create_minimal_readme() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let manager = create_test_manager();
        
        let summary = "This is a test project for demonstrating functionality.";
        manager.create_minimal_readme(temp_dir.path(), summary).await?;
        
        let readme_path = temp_dir.path().join("README.md");
        assert!(readme_path.exists());
        
        let content = fs::read_to_string(&readme_path)?;
        assert!(content.contains(summary));
        assert!(content.contains("# "));
        assert!(content.contains("## Installation"));
        assert!(content.contains("## Usage"));
        
        Ok(())
    }
}