use thiserror::Error;

pub type Result<T> = std::result::Result<T, DocTreeError>;

#[derive(Error, Debug)]
pub enum DocTreeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("OpenAI API error: {0}")]
    OpenAi(#[from] async_openai::error::OpenAIError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Scanner error: {0}")]
    Scanner(String),

    #[error("Summarizer error: {0}")]
    Summarizer(String),

    #[error("README error: {0}")]
    Readme(String),

    #[error("Path error: {0}")]
    Path(String),

    #[error("Environment variable error: {variable}")]
    EnvironmentVariable { variable: String },

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl DocTreeError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn cache(msg: impl Into<String>) -> Self {
        Self::Cache(msg.into())
    }

    pub fn scanner(msg: impl Into<String>) -> Self {
        Self::Scanner(msg.into())
    }

    pub fn summarizer(msg: impl Into<String>) -> Self {
        Self::Summarizer(msg.into())
    }

    pub fn readme(msg: impl Into<String>) -> Self {
        Self::Readme(msg.into())
    }

    pub fn path(msg: impl Into<String>) -> Self {
        Self::Path(msg.into())
    }

    pub fn environment_variable(variable: impl Into<String>) -> Self {
        Self::EnvironmentVariable {
            variable: variable.into(),
        }
    }

    pub fn unknown(msg: impl Into<String>) -> Self {
        Self::Unknown(msg.into())
    }
}