# DocTreeAI

DocTreeAI is a Rust-based command-line tool that validates README.md files against your codebase using hierarchical tree-based summarization with local Language Models (LLMs). The tool intelligently scans your codebase, generates summaries for files and directories in a bottom-up fashion, and validates that your README accurately reflects the current state of your code by suggesting updates when documentation becomes outdated.

## Features

- **🌳 Hierarchical Summarization**: Uses tree-based analysis starting from individual files up to the project root
- **📦 Cache System**: Efficient SHA-256 based caching to avoid redundant API calls  
- **🔄 Smart README Validation**: Validates README content against current codebase and suggests updates when needed
- **🚫 .gitignore Integration**: Respects your project's ignore patterns
- **🔌 Local LLM Support**: Works with any OpenAI-compatible local model server
- **⚡ Fast Performance**: Concurrent processing and intelligent caching for speed
- **📊 Progress Tracking**: Detailed logging and cache statistics

## Installation

### Prerequisites

- Rust (latest stable edition)
- A running local LLM server compatible with OpenAI API (we **strongly recommend** OpenAI's GPT-OSS-20B model)

### Build from Source

```bash
git clone <repository-url>
cd doctreeai
cargo build --release
```

The binary will be available at `target/release/doctreeai`.

## Configuration

DocTreeAI uses environment variables for configuration. We **highly recommend** using OpenAI's GPT-OSS-20B model for optimal documentation generation:

```bash
# Recommended Configuration with GPT-OSS-20B
export OPENAI_API_BASE="http://localhost:11434/v1"  # Your local LLM endpoint
export OPENAI_MODEL_NAME="gpt-oss-20b"             # OpenAI's GPT-OSS-20B (recommended)

# Optional
export OPENAI_API_KEY="ollama"                     # API key (can be placeholder)
export DOCTREEAI_CACHE_DIR=".doctreeai_cache"      # Custom cache directory
export DOCTREEAI_LOG_LEVEL="info"                  # Logging level
```

### Why GPT-OSS-20B?

We strongly recommend OpenAI's **GPT-OSS-20B** model for DocTreeAI because:

- **🧠 Superior Code Analysis**: Excels at understanding and explaining code across all programming languages
- **📝 Documentation Excellence**: Specifically optimized for generating clear, comprehensive technical documentation
- **🔧 Advanced Reasoning**: Provides full chain-of-thought reasoning for better documentation quality
- **⚡ Efficient Performance**: Only 3.6B active parameters per token, runs smoothly on 16GB consumer GPUs
- **🛠 Tool Integration**: Native support for structured outputs and function calling
- **🎯 Cost-Effective**: Optimized for local deployment with minimal resource requirements
- **📊 Proven Results**: Matches or exceeds larger models on coding and technical analysis benchmarks
- **🔓 Open Source**: Available under Apache 2.0 license for commercial and personal use

## Usage

### Commands

```bash
# Initialize DocTreeAI in a project
doctreeai init

# Validate README and suggest updates
doctreeai run

# Force regeneration (ignore cache)
doctreeai run --force

# Dry run (preview without changes)
doctreeai run --dry-run

# Show project and cache information
doctreeai info

# Test LLM connection
doctreeai test

# Clean cache
doctreeai clean

# Enable verbose logging
doctreeai -v run
```

### Workflow

1. **Initialize**: Run `doctreeai init` to set up the cache and update .gitignore
2. **Configure**: Set your environment variables for the local LLM
3. **Validate**: Run `doctreeai run` to validate your README.md and get update suggestions
4. **Iterate**: The tool will use cached summaries for unchanged files on subsequent runs

## How It Works

### Hierarchical Analysis

DocTreeAI performs a bottom-up analysis of your codebase:

1. **File Level**: Each source code file is analyzed and summarized
2. **Directory Level**: Directory summaries are created from child summaries  
3. **Project Level**: The root summary becomes your project overview

### Caching Strategy

DocTreeAI uses a directory-mirrored cache structure for optimal performance:

- **File-Level Caching**: Each source file gets a corresponding `.summary` file in the cache
- **Directory Summaries**: Directories have `.dir_summary` files containing their aggregated summaries
- **Structure Mirroring**: Cache directory structure exactly matches your codebase structure
- **SHA-256 Hashing**: Files are hashed to detect changes and invalidate specific cache entries
- **Incremental Updates**: Only modified files trigger new LLM API calls
- **Small Context Windows**: Each cache file is independent, reducing memory usage

Example cache structure:
```
.doctreeai_cache/
├── src/
│   ├── main.rs.summary
│   ├── lib.rs.summary
│   ├── cache.rs.summary
│   └── .dir_summary
├── tests/
│   ├── integration_tests.rs.summary
│   └── .dir_summary
└── .dir_summary
```

### Intelligent README Validation

- **Line Mapping**: Maps README content lines to relevant cached documentation
- **Change Detection**: Identifies when code changes affect specific README sections
- **Smart Suggestions**: Provides targeted update suggestions without modifying your files
- **Mapping Persistence**: Tracks line-to-cache mappings in `.doctreeai_cache/readme_mapping.json`

### Validation Mapping System

DocTreeAI uses a sophisticated mapping system to ensure every line in your README that describes code can be validated:

1. **Content Analysis**: The tool analyzes each line in your README to identify references to code components
2. **Cache Mapping**: Lines mentioning modules, functions, files, or directories are mapped to relevant cache entries
3. **Change Tracking**: When cached documentation is invalidated (due to code changes), the tool identifies affected README lines
4. **Validation Process**: 
   - Compares current README content against the latest code summaries
   - Detects outdated or inaccurate descriptions
   - Generates specific suggestions for lines that need updating
5. **Non-Invasive**: All suggestions are presented to the user without modifying the README file

This approach ensures your documentation stays accurate while giving you full control over what changes to accept.

## Supported File Types

DocTreeAI analyzes the following file types:

- **Languages**: Rust, Python, JavaScript/TypeScript, Go, Java, C/C++, C#, PHP, Ruby, Swift, Kotlin, and more
- **Web**: HTML, CSS, SCSS, Vue, Svelte
- **Config**: JSON, YAML, TOML, XML  
- **Documentation**: Markdown, LaTeX, reStructuredText
- **Scripts**: Shell scripts, PowerShell
- **Other**: SQL, GraphQL, Protocol Buffers, Dockerfiles, Makefiles

## Architecture

The tool consists of several key modules:

- **Scanner**: Gitignore-aware directory traversal and file discovery
- **Hasher**: SHA-256 file content hashing for change detection
- **Cache**: Directory-mirrored caching system with individual summary files
- **LLM Client**: OpenAI-compatible API integration with retry logic
- **Summarizer**: Hierarchical tree-based summarization engine
- **README Validator**: Validates README against codebase and suggests updates

## Development

### Running Tests

```bash
cargo test
```

### Linting

```bash
cargo clippy
```

### Local Development

1. Set up a local LLM server with GPT-OSS-20B:
   ```bash
   # Using Ollama (recommended)
   ollama pull gpt-oss-20b
   ollama serve
   
   # Or using LM Studio - download openai/gpt-oss-20b from the model library
   ```
2. Configure environment variables (see Configuration section)
3. Run `cargo run -- test` to verify setup
4. Use `cargo run -- run --dry-run` to test without modifications

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests for new functionality  
5. Run `cargo test` and `cargo clippy`
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Troubleshooting

### Common Issues

**LLM Connection Failed**
- Ensure your local LLM server is running
- Verify the `OPENAI_API_BASE` URL is correct
- Check that GPT-OSS-20B model is available: `ollama list` or check LM Studio model library
- For first-time setup: `ollama pull gpt-oss-20b`

**Permission Denied**
- Ensure the tool has write permissions for the target directory
- Check that `.doctreeai_cache` is not read-only

**Out of Memory**
- For very large codebases, try processing subdirectories individually
- Increase your local LLM's context window if possible

### Getting Help

- Use `doctreeai info` to check configuration and cache status
- Use `doctreeai test` to verify LLM connectivity
- Enable verbose logging with `-v` flag for detailed output

---

*Generated with DocTreeAI - AI-powered documentation that stays up-to-date* 🤖