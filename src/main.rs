use clap::{Parser, Subcommand};
use doctreeai::{
    cache::CacheManager,
    config::Config, 
    error::Result,
    llm::LanguageModelClient,
    readme::ReadmeManager,
    readme_validator::ReadmeValidator,
    summarizer::HierarchicalSummarizer,
};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "doctreeai")]
#[command(about = "A CLI tool that automates generation and updating of README.md using hierarchical tree-based summarization")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(short, long, global = true, help = "Enable verbose logging")]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Initialize the cache and update .gitignore")]
    Init {
        #[arg(short, long, help = "Target directory path")]
        path: Option<PathBuf>,
    },
    #[command(about = "Execute the main documentation generation and update logic")]
    Run {
        #[arg(short, long, help = "Target directory path")]
        path: Option<PathBuf>,
        #[arg(long, help = "Ignore all cached content and regenerate all summaries from scratch")]
        force: bool,
        #[arg(long, help = "Show the tree structure and summaries without updating README")]
        dry_run: bool,
    },
    #[command(about = "Remove the .doctreeai_cache/ directory")]
    Clean {
        #[arg(short, long, help = "Target directory path")]
        path: Option<PathBuf>,
    },
    #[command(about = "Show information about the current README and cache")]
    Info {
        #[arg(short, long, help = "Target directory path")]
        path: Option<PathBuf>,
    },
    #[command(about = "Test connection to the configured LLM")]
    Test {
        #[arg(short, long, help = "Target directory path")]
        path: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    if cli.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    }
    
    match &cli.command {
        Commands::Init { path } => {
            let target_path = path.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
            init_command(&target_path).await
        }
        Commands::Run { path, force, dry_run } => {
            let target_path = path.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
            run_command(&target_path, *force, *dry_run).await
        }
        Commands::Clean { path } => {
            let target_path = path.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
            clean_command(&target_path).await
        }
        Commands::Info { path } => {
            let target_path = path.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
            info_command(&target_path).await
        }
        Commands::Test { path: _ } => {
            test_command().await
        }
    }
}

async fn init_command(path: &Path) -> Result<()> {
    println!("🚀 Initializing DocTreeAI in: {}", path.display());
    
    let config = Config::load()?;
    config.validate()?;
    
    // Initialize cache manager and create cache directory
    let cache_manager = CacheManager::new(path, &config.cache_dir_name)?;
    cache_manager.initialize_cache_directory()?;
    
    println!("✅ Cache directory initialized");
    println!("✅ Added {} to .gitignore", config.cache_dir_name);
    println!("\n🎯 Ready to run! Use 'doctreeai run' to generate documentation.");
    
    Ok(())
}

async fn run_command(path: &Path, force: bool, dry_run: bool) -> Result<()> {
    println!("🔍 Running DocTreeAI on: {}", path.display());
    if force {
        println!("⚡ Force mode enabled - regenerating all summaries");
    }
    if dry_run {
        println!("🔍 Dry run mode - will not update README.md");
    }
    
    let config = Config::load()?;
    config.validate()?;
    
    // Initialize components
    let llm_client = LanguageModelClient::new(&config)?;
    let cache_manager = CacheManager::new(path, &config.cache_dir_name)?;
    
    // Test LLM connection first
    println!("🧠 Testing LLM connection...");
    if let Err(e) = llm_client.test_connection().await {
        eprintln!("❌ LLM connection failed: {e}");
        eprintln!("💡 Make sure your local LLM server is running and environment variables are set correctly:");
        eprintln!("   OPENAI_API_BASE={}", config.openai_api_base);
        eprintln!("   OPENAI_MODEL_NAME={}", config.openai_model_name);
        return Err(e);
    }
    println!("✅ LLM connection successful");
    
    // Create summarizer and generate project summary
    let llm_client_2 = LanguageModelClient::new(&config)?;
    let cache_manager_2 = CacheManager::new(path, &config.cache_dir_name)?;
    let mut summarizer = HierarchicalSummarizer::new(llm_client, cache_manager, force);
    
    println!("📊 Generating hierarchical project summary...");
    let project_summary = summarizer.generate_project_summary(path).await?;
    
    let (cache_entries, cache_size) = summarizer.get_cache_stats();
    println!("📊 Cache stats: {cache_entries} entries, {cache_size} bytes");
    
    if dry_run {
        println!("\n📋 Generated Project Summary:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("{project_summary}");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🔍 Dry run complete - README.md was not modified");
        return Ok(());
    }
    
    // Validate README.md against cache
    println!("📝 Validating README.md against current codebase...");
    let mut readme_validator = ReadmeValidator::new(cache_manager_2, llm_client_2);
    let validation_results = readme_validator.validate_readme(path, &project_summary).await?;
    
    ReadmeValidator::print_validation_results(&validation_results);
    
    if validation_results.is_empty() {
        println!("✅ README.md validation completed - no updates needed!");
    } else {
        println!("✅ README.md validation completed - {} suggestions generated!", validation_results.len());
        println!("💡 Review the suggestions above and update your README.md accordingly");
    }
    
    Ok(())
}

async fn clean_command(path: &Path) -> Result<()> {
    println!("🧹 Cleaning DocTreeAI cache in: {}", path.display());
    
    let config = Config::load()?;
    let mut cache_manager = CacheManager::new(path, &config.cache_dir_name)?;
    
    cache_manager.clear_cache()?;
    println!("✅ Cache directory removed");
    
    Ok(())
}

async fn info_command(path: &Path) -> Result<()> {
    println!("ℹ️  DocTreeAI Information for: {}", path.display());
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let config = Config::load()?;
    config.validate()?;
    
    // Configuration info
    println!("📋 Configuration:");
    println!("  API Base: {}", config.openai_api_base);
    println!("  Model: {}", config.openai_model_name);
    println!("  Cache Dir: {}", config.cache_dir_name);
    println!();
    
    // Cache info
    let cache_manager = CacheManager::new(path, &config.cache_dir_name)?;
    let (cache_entries, cache_size) = cache_manager.get_cache_stats();
    println!("💾 Cache Information:");
    println!("  Entries: {cache_entries}");
    println!("  Size: {cache_size} bytes");
    println!("  Valid: {}", cache_manager.is_cache_valid());
    println!();
    
    // README info
    let readme_manager = ReadmeManager::new();
    let readme_info = readme_manager.get_readme_info(path)?;
    
    println!("📄 README Information:");
    readme_info.print_summary();
    
    Ok(())
}

async fn test_command() -> Result<()> {
    println!("🧪 Testing DocTreeAI configuration...");
    
    let config = Config::load()?;
    println!("✅ Configuration loaded successfully");
    
    config.validate()?;
    println!("✅ Configuration validation passed");
    
    let llm_client = LanguageModelClient::new(&config)?;
    println!("✅ LLM client created");
    
    println!("🧠 Testing LLM connection...");
    match llm_client.test_connection().await {
        Ok(()) => {
            println!("✅ LLM connection test passed");
            println!("🎉 All tests passed! DocTreeAI is ready to use.");
        }
        Err(e) => {
            eprintln!("❌ LLM connection test failed: {e}");
            eprintln!("💡 Troubleshooting tips:");
            eprintln!("   1. Make sure your local LLM server is running");
            eprintln!("   2. Verify the API base URL: {}", config.openai_api_base);
            eprintln!("   3. Check the model name: {}", config.openai_model_name);
            eprintln!("   4. Ensure the API key is set (can be placeholder for local models)");
            return Err(e);
        }
    }
    
    Ok(())
}
