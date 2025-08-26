use crate::config::Config;
use crate::error::{DocTreeError, Result};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
        CreateChatCompletionRequest, Role,
    },
    Client,
};
use std::path::Path;
use tokio::time::{sleep, Duration};

pub struct LanguageModelClient {
    client: Client<OpenAIConfig>,
    model_name: String,
    max_retries: u32,
    retry_delay: Duration,
}

impl LanguageModelClient {
    pub fn new(config: &Config) -> Result<Self> {
        let openai_config = OpenAIConfig::new()
            .with_api_base(config.openai_api_base.clone())
            .with_api_key(config.openai_api_key.clone());

        let client = Client::with_config(openai_config);

        Ok(Self {
            client,
            model_name: config.openai_model_name.clone(),
            max_retries: 3,
            retry_delay: Duration::from_secs(2),
        })
    }

    pub async fn generate_file_summary(&self, file_path: &Path, content: &str) -> Result<String> {
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let prompt = format!(
            "Analyze this source code file and provide a concise, one-paragraph Markdown summary of its purpose and primary functions. File: {filename}\n\nCode:\n```\n{content}\n```"
        );

        self.generate_completion(&prompt).await
    }

    pub async fn generate_directory_summary(
        &self,
        directory_name: &str,
        children_summaries: &[String],
    ) -> Result<String> {
        let combined_summaries = children_summaries.join("\n\n");

        let prompt = format!(
            "Analyze the following summaries of the contents of the '{directory_name}' directory. Provide a concise, one-paragraph Markdown summary of the directory's overall role and components.\n\nSummaries:\n{combined_summaries}"
        );

        self.generate_completion(&prompt).await
    }

    pub async fn update_readme(
        &self,
        existing_readme: &str,
        project_summary: &str,
    ) -> Result<String> {
        let prompt = format!(
            "Update the following README.md file with the new project summary provided. Preserve the existing structure, tone, and any manually created sections (like \"Installation\" or \"Usage\"). Integrate the new summary into the relevant part of the document that describes what the project does.\n\n**Existing README.md:**\n---\n{existing_readme}\n---\n\n**New Project Summary:**\n---\n{project_summary}\n---"
        );

        self.generate_completion(&prompt).await
    }

    pub async fn create_new_readme(
        &self,
        project_summary: &str,
        project_name: &str,
    ) -> Result<String> {
        let prompt = format!(
            "Create a new README.md file for a project called '{project_name}' using the following project summary. Include standard sections like installation, usage, and contributing guidelines where appropriate.\n\n**Project Summary:**\n{project_summary}"
        );

        self.generate_completion(&prompt).await
    }

    async fn generate_completion(&self, prompt: &str) -> Result<String> {
        let mut attempt = 0;

        loop {
            match self.try_generate_completion(prompt).await {
                Ok(response) => return Ok(response),
                Err(e) if attempt < self.max_retries => {
                    attempt += 1;
                    log::warn!(
                        "LLM API call failed (attempt {}/{}): {}",
                        attempt,
                        self.max_retries + 1,
                        e
                    );
                    sleep(self.retry_delay * attempt).await;
                    continue;
                }
                Err(e) => {
                    return Err(DocTreeError::summarizer(format!(
                        "LLM API failed after {} retries: {}",
                        self.max_retries + 1,
                        e
                    )));
                }
            }
        }
    }

    async fn try_generate_completion(&self, prompt: &str) -> Result<String> {
        let messages = vec![
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                role: Role::System,
                content: Some("You are a helpful assistant that generates concise, accurate documentation. Always respond in Markdown format. Focus on clarity and brevity.".to_string()),
                name: None,
            }),
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                role: Role::User,
                content: Some(ChatCompletionRequestUserMessageContent::Text(prompt.to_string())),
                name: None,
            }),
        ];

        let request = CreateChatCompletionRequest {
            model: self.model_name.clone(),
            messages,
            max_tokens: Some(1000),
            temperature: Some(0.3),
            top_p: Some(0.9),
            n: Some(1),
            stream: Some(false),
            stop: None,
            presence_penalty: Some(0.0),
            frequency_penalty: Some(0.0),
            logit_bias: None,
            user: None,
            response_format: None,
            seed: None,
            tools: None,
            tool_choice: None,
            #[allow(deprecated)]
            function_call: None,
            #[allow(deprecated)]
            functions: None,
        };

        log::debug!("Sending request to LLM with model: {}", self.model_name);

        let response = self.client.chat().create(request).await?;

        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .ok_or_else(|| DocTreeError::summarizer("No response content from LLM"))?;

        log::debug!("Received LLM response: {} characters", content.len());

        Ok(content.trim().to_string())
    }

    pub async fn test_connection(&self) -> Result<()> {
        log::info!("Testing LLM connection...");

        let test_prompt = "Respond with exactly: 'Connection test successful'";

        match self.generate_completion(test_prompt).await {
            Ok(response) => {
                log::info!("LLM connection test successful. Response: {response}");
                Ok(())
            }
            Err(e) => {
                log::error!("LLM connection test failed: {e}");
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    async fn create_test_client() -> LanguageModelClient {
        // Note: These tests require a running local LLM server
        // Load configuration from environment variables
        let config = Config::load().unwrap();

        LanguageModelClient::new(&config).unwrap()
    }

    #[tokio::test]
    #[ignore] // Requires local LLM server
    async fn test_generate_file_summary() {
        let client = create_test_client().await;
        let content = "fn main() { println!(\"Hello, world!\"); }";
        let path = Path::new("main.rs");

        let result = client.generate_file_summary(path, content).await;
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert!(!summary.is_empty());
        assert!(summary.len() > 10);
    }

    #[tokio::test]
    #[ignore] // Requires local LLM server
    async fn test_generate_directory_summary() {
        let client = create_test_client().await;
        let summaries = vec![
            "A main function that prints hello world".to_string(),
            "A utility module with helper functions".to_string(),
        ];

        let result = client.generate_directory_summary("src", &summaries).await;
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert!(!summary.is_empty());
        assert!(summary.len() > 10);
    }

    #[tokio::test]
    #[ignore] // Requires local LLM server
    async fn test_connection() {
        let client = create_test_client().await;
        let result = client.test_connection().await;
        // This may fail if no LLM server is running, which is expected in CI
        println!("Connection test result: {result:?}");
    }
}
