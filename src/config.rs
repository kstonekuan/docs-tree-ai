use crate::error::{DocTreeError, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub openai_api_base: String,
    pub openai_api_key: String,
    pub openai_model_name: String,
    pub cache_dir_name: String,
    pub log_level: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        // Load .env file if it exists (ignore errors if not found)
        let _ = dotenvy::dotenv();

        // API base URL is required - no default
        let openai_api_base = env::var("OPENAI_API_BASE")
            .or_else(|_| env::var("OPENAI_BASE_URL"))
            .map_err(|_| {
                DocTreeError::config(
                    "OPENAI_API_BASE or OPENAI_BASE_URL environment variable is required",
                )
            })?;

        // API key can default to "local" for local model instances
        let openai_api_key = env::var("OPENAI_API_KEY").unwrap_or_else(|_| "local".to_string());

        // Model name is required - no default
        let openai_model_name = env::var("OPENAI_MODEL_NAME")
            .or_else(|_| env::var("OPENAI_MODEL"))
            .map_err(|_| {
                DocTreeError::config(
                    "OPENAI_MODEL_NAME or OPENAI_MODEL environment variable is required",
                )
            })?;

        let cache_dir_name =
            env::var("DOCTREEAI_CACHE_DIR").unwrap_or_else(|_| ".doctreeai_cache".to_string());

        let log_level = env::var("DOCTREEAI_LOG_LEVEL")
            .or_else(|_| env::var("LOG_LEVEL"))
            .unwrap_or_else(|_| "info".to_string());

        Ok(Config {
            openai_api_base,
            openai_api_key,
            openai_model_name,
            cache_dir_name,
            log_level,
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.openai_api_base.is_empty() {
            return Err(DocTreeError::config("OPENAI_API_BASE cannot be empty"));
        }

        if self.openai_model_name.is_empty() {
            return Err(DocTreeError::config("OPENAI_MODEL_NAME cannot be empty"));
        }

        if self.cache_dir_name.is_empty() {
            return Err(DocTreeError::config("Cache directory name cannot be empty"));
        }

        if !self.openai_api_base.starts_with("http://")
            && !self.openai_api_base.starts_with("https://")
        {
            return Err(DocTreeError::config(
                "OPENAI_API_BASE must be a valid HTTP/HTTPS URL",
            ));
        }

        log::info!("Configuration loaded successfully:");
        log::info!("  API Base: {}", self.openai_api_base);
        log::info!("  Model: {}", self.openai_model_name);
        log::info!("  Cache Dir: {}", self.cache_dir_name);
        log::info!("  Log Level: {}", self.log_level);

        Ok(())
    }

    pub fn get_cache_dir_path(&self, base_path: &std::path::Path) -> std::path::PathBuf {
        base_path.join(&self.cache_dir_name)
    }
}
