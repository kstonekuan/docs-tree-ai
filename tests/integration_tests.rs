use doctreeai::{
    cache::CacheManager,
    config::Config,
    hasher::FileHasher,
    scanner::{DirectoryScanner, FileNode},
};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_file_hasher() -> doctreeai::Result<()> {
    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("test.txt");
    
    fs::write(&file_path, "Hello, world!")?;
    
    let hash1 = FileHasher::compute_file_hash(&file_path)?;
    let hash2 = FileHasher::compute_file_hash(&file_path)?;
    
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64); // SHA-256 produces 64-char hex string
    
    Ok(())
}

#[test]
fn test_directory_scanner() -> doctreeai::Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = temp_dir.path();
    
    // Create test directory structure
    fs::create_dir_all(base_path.join("src"))?;
    fs::create_dir_all(base_path.join("tests"))?;
    fs::write(base_path.join("src/main.rs"), "fn main() {}")?;
    fs::write(base_path.join("src/lib.rs"), "pub mod utils;")?;
    fs::write(base_path.join("tests/test.rs"), "#[test] fn test() {}")?;
    fs::write(base_path.join("README.md"), "# Test Project")?;
    
    let scanner = DirectoryScanner::new(base_path.to_path_buf());
    let root_node = scanner.scan_directory()?;
    
    assert!(root_node.is_directory);
    assert!(!root_node.children.is_empty());
    
    let source_files = DirectoryScanner::filter_source_files(&root_node);
    assert!(!source_files.is_empty());
    
    // Should find our source files
    let source_file_names: Vec<_> = source_files
        .iter()
        .filter_map(|node| node.path.file_name().and_then(|n| n.to_str()))
        .collect();
    
    assert!(source_file_names.contains(&"main.rs"));
    assert!(source_file_names.contains(&"lib.rs"));
    assert!(source_file_names.contains(&"test.rs"));
    assert!(source_file_names.contains(&"README.md"));
    
    Ok(())
}

#[test]
fn test_cache_manager() -> doctreeai::Result<()> {
    let temp_dir = TempDir::new()?;
    let mut cache = CacheManager::new(temp_dir.path(), ".test_cache")?;
    
    let test_path = temp_dir.path().join("test.rs");
    fs::write(&test_path, "fn test() {}")?;
    
    let hash = FileHasher::compute_file_hash(&test_path)?;
    let summary = "A test function".to_string();
    
    // Store summary
    cache.store_summary(&test_path, hash.clone(), summary.clone())?;
    
    // Retrieve summary
    let retrieved = cache.get_cached_summary(&test_path, &hash);
    assert_eq!(retrieved, Some(summary));
    
    // Test cache miss with different hash
    let wrong_hash = "wrong_hash";
    let not_found = cache.get_cached_summary(&test_path, wrong_hash);
    assert_eq!(not_found, None);
    
    // Cache is automatically persisted when store_summary is called
    
    let cache2 = CacheManager::new(temp_dir.path(), ".test_cache")?;
    let retrieved_after_reload = cache2.get_cached_summary(&test_path, &hash);
    assert_eq!(retrieved_after_reload, Some("A test function".to_string()));
    
    Ok(())
}

#[test]
fn test_config_loading() {
    let config = Config::load();
    assert!(config.is_ok());
    
    let config = config.unwrap();
    assert!(!config.openai_api_base.is_empty());
    assert!(!config.openai_model_name.is_empty());
    assert!(!config.cache_dir_name.is_empty());
}

#[test]
fn test_file_node_operations() {
    let mut parent = FileNode::new("/tmp/parent".into(), true);
    let child1 = FileNode::new("/tmp/parent/file1.rs".into(), false);
    let child2 = FileNode::new("/tmp/parent/file2.rs".into(), false);
    
    parent.add_child(child1);
    parent.add_child(child2);
    
    assert_eq!(parent.children.len(), 2);
    assert!(parent.is_directory);
    
    // Test source code file detection
    let rust_file = FileNode::new("test.rs".into(), false);
    assert!(rust_file.is_source_code_file());
    
    let python_file = FileNode::new("test.py".into(), false);
    assert!(python_file.is_source_code_file());
    
    let binary_file = FileNode::new("test.bin".into(), false);
    assert!(!binary_file.is_source_code_file());
    
    let directory = FileNode::new("src".into(), true);
    assert!(!directory.is_source_code_file());
}

#[tokio::test]
async fn test_end_to_end_workflow() -> doctreeai::Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = temp_dir.path();
    
    // Create a simple project structure
    fs::create_dir_all(base_path.join("src"))?;
    fs::write(
        base_path.join("src/main.rs"),
        r#"
fn main() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
    )?;
    
    fs::write(
        base_path.join("src/utils.rs"),
        r#"
pub fn helper_function() -> String {
    "Helper".to_string()
}
"#,
    )?;
    
    // Test directory scanning
    let scanner = DirectoryScanner::new(base_path.to_path_buf());
    let root_node = scanner.scan_directory()?;
    
    assert!(root_node.is_directory);
    let source_files = DirectoryScanner::filter_source_files(&root_node);
    assert_eq!(source_files.len(), 2);
    
    // Test cache initialization
    let config = Config::load()?;
    let cache_manager = CacheManager::new(base_path, &config.cache_dir_name)?;
    cache_manager.initialize_cache_directory()?;
    
    let cache_path = base_path.join(&config.cache_dir_name);
    assert!(cache_path.exists());
    
    // Test gitignore creation
    let gitignore_path = base_path.join(".gitignore");
    assert!(gitignore_path.exists());
    
    let gitignore_content = fs::read_to_string(&gitignore_path)?;
    assert!(gitignore_content.contains(&config.cache_dir_name));
    
    Ok(())
}

#[test]
fn test_gitignore_patterns() -> doctreeai::Result<()> {
    let temp_dir = TempDir::new()?;
    let base_path = temp_dir.path();
    
    // Create various files and directories
    fs::create_dir_all(base_path.join("src"))?;
    fs::create_dir_all(base_path.join("target"))?;
    fs::create_dir_all(base_path.join("node_modules"))?;
    fs::create_dir_all(base_path.join(".git"))?;
    fs::create_dir_all(base_path.join(".doctreeai_cache"))?;
    
    fs::write(base_path.join("src/main.rs"), "fn main() {}")?;
    fs::create_dir_all(base_path.join("target/debug"))?;
    fs::create_dir_all(base_path.join("node_modules/package"))?;
    fs::create_dir_all(base_path.join(".git"))?;
    fs::write(base_path.join("target/debug/app"), "binary")?;
    fs::write(base_path.join("node_modules/package/index.js"), "module")?;
    fs::write(base_path.join(".git/config"), "git config")?;
    fs::write(base_path.join(".doctreeai_cache/cache.json"), "cache")?;
    
    let scanner = DirectoryScanner::new(base_path.to_path_buf());
    let root_node = scanner.scan_directory()?;
    
    // Should find source files but ignore build artifacts and cache
    let all_files: Vec<_> = collect_all_files(&root_node);
    let file_names: Vec<_> = all_files
        .iter()
        .filter_map(|path| path.file_name().and_then(|n| n.to_str()))
        .collect();
    
    // Should find the source file
    assert!(file_names.contains(&"main.rs"));
    
    // Should not find ignored files/directories
    assert!(!file_names.contains(&"app"));
    assert!(!file_names.contains(&"index.js"));
    assert!(!file_names.contains(&"config"));
    assert!(!file_names.contains(&"cache.json"));
    
    Ok(())
}

fn collect_all_files(node: &FileNode) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    
    if !node.is_directory {
        files.push(node.path.clone());
    }
    
    for child in &node.children {
        files.extend(collect_all_files(child));
    }
    
    files
}