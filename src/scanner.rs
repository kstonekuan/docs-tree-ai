use crate::error::{DocTreeError, Result};
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub is_directory: bool,
    pub children: Vec<FileNode>,
    pub content_hash: Option<String>,
    pub summary: Option<String>,
}

impl FileNode {
    pub fn new(path: PathBuf, is_directory: bool) -> Self {
        Self {
            path,
            is_directory,
            children: Vec::new(),
            content_hash: None,
            summary: None,
        }
    }

    pub fn add_child(&mut self, child: FileNode) {
        self.children.push(child);
    }

    pub fn get_relative_path(&self, base: &Path) -> Result<PathBuf> {
        pathdiff::diff_paths(&self.path, base)
            .ok_or_else(|| DocTreeError::path("Failed to compute relative path"))
    }

    pub fn is_source_code_file(&self) -> bool {
        if self.is_directory {
            return false;
        }

        let extension = self.path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        matches!(extension.to_lowercase().as_str(),
            "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "java" | "cpp" | "c" | "h" | "hpp" |
            "cs" | "php" | "rb" | "swift" | "kt" | "scala" | "clj" | "hs" | "elm" | "dart" |
            "r" | "jl" | "ml" | "fs" | "pl" | "sh" | "bash" | "zsh" | "fish" | "ps1" |
            "html" | "css" | "scss" | "sass" | "less" | "vue" | "svelte" | "xml" | "yaml" | "yml" |
            "json" | "toml" | "ini" | "cfg" | "conf" | "dockerfile" | "makefile" | "cmake" |
            "sql" | "graphql" | "proto" | "thrift" | "avro" | "md" | "mdx" | "tex" | "rst"
        )
    }
}

pub struct DirectoryScanner {
    base_path: PathBuf,
}

impl DirectoryScanner {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub fn scan_directory(&self) -> Result<FileNode> {
        log::info!("Starting directory scan of: {}", self.base_path.display());

        let mut root = FileNode::new(self.base_path.clone(), true);
        let mut path_to_node: HashMap<PathBuf, Vec<FileNode>> = HashMap::new();

        let walker = WalkBuilder::new(&self.base_path)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .require_git(false)
            .follow_links(false)
            .same_file_system(true)
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    let path = entry.path();
                    let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

                    if path == self.base_path {
                        continue;
                    }

                    if self.should_skip_path(path) {
                        continue;
                    }

                    let node = FileNode::new(path.to_path_buf(), is_dir);
                    
                    if let Some(parent_path) = path.parent() {
                        path_to_node.entry(parent_path.to_path_buf())
                            .or_default()
                            .push(node);
                    }
                }
                Err(err) => {
                    log::warn!("Error walking directory: {err}");
                    continue;
                }
            }
        }

        Self::build_tree(&mut root, &mut path_to_node)?;

        log::info!("Directory scan completed. Found {} total items", Self::count_nodes(&root));
        
        Ok(root)
    }

    fn should_skip_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        
        if path_str.contains(".doctreeai_cache") {
            return true;
        }
        
        if path_str.contains(".git/") {
            return true;
        }

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if file_name.starts_with('.') && file_name != ".gitignore" {
                return true;
            }

            if matches!(file_name, 
                "node_modules" | "target" | "build" | "dist" | "out" | "__pycache__" | 
                ".pytest_cache" | ".mypy_cache" | ".tox" | ".coverage" | "coverage" |
                ".venv" | "venv" | "env" | ".env"
            ) {
                return true;
            }
        }

        false
    }

    fn build_tree(parent: &mut FileNode, path_to_children: &mut HashMap<PathBuf, Vec<FileNode>>) -> Result<()> {
        if let Some(children) = path_to_children.remove(&parent.path) {
            for mut child in children {
                if child.is_directory {
                    Self::build_tree(&mut child, path_to_children)?;
                }
                parent.add_child(child);
            }
        }

        parent.children.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.path.cmp(&b.path),
            }
        });

        Ok(())
    }

    fn count_nodes(node: &FileNode) -> usize {
        1 + node.children.iter().map(Self::count_nodes).sum::<usize>()
    }

    pub fn filter_source_files(node: &FileNode) -> Vec<&FileNode> {
        let mut result = Vec::new();
        
        if !node.is_directory && node.is_source_code_file() {
            result.push(node);
        }

        for child in &node.children {
            result.extend(Self::filter_source_files(child));
        }

        result
    }

    pub fn get_directories(node: &FileNode) -> Vec<&FileNode> {
        let mut result = Vec::new();
        
        if node.is_directory {
            result.push(node);
        }

        for child in &node.children {
            result.extend(Self::get_directories(child));
        }

        result
    }
}