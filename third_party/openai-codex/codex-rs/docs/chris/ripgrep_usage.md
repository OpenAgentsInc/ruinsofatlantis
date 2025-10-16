# Ripgrep Usage in Codex

This document details how ripgrep (`rg`) is integrated and used within the Codex codebase.

## Overview

Ripgrep is a line-oriented search tool that recursively searches directories for a regex pattern. Codex leverages ripgrep in multiple ways:

1. **Direct command execution** - As a safe command for searching
2. **Library integration** - Using ripgrep's `ignore` crate for file traversal
3. **Prompt guidance** - Recommended as the preferred search tool

## 1. Safe Command Execution

### Security Policy Configuration

Location: `codex-rs/execpolicy/src/default.policy:103-124`

Ripgrep is configured as a safe command that can be executed without user approval:

```python
define_program(
    program="rg",
    options=[
        opt("-A", ARG_POS_INT),        # Lines after match
        opt("-B", ARG_POS_INT),        # Lines before match
        opt("-C", ARG_POS_INT),        # Context lines (before & after)
        opt("-d", ARG_POS_INT),        # Max depth
        opt("--max-depth", ARG_POS_INT),
        opt("-g", ARG_OPAQUE_VALUE),   # Glob patterns
        opt("--glob", ARG_OPAQUE_VALUE),
        opt("-m", ARG_POS_INT),        # Max matches
        opt("--max-count", ARG_POS_INT),
        flag("-n"),                    # Line numbers
        flag("-i"),                    # Case insensitive
        flag("-l"),                    # Files with matches
        flag("--files"),               # List all files
        flag("--files-with-matches"),
        flag("--files-without-match"),
    ],
    args=[ARG_OPAQUE_VALUE, ARG_RFILES_OR_CWD],
)
```

### Safety Checks

Location: `codex-rs/core/src/is_safe_command.rs:157-185`

The codebase includes security checks to prevent dangerous ripgrep options:

```rust
// Unsafe ripgrep options that are blocked:
const UNSAFE_RIPGREP_OPTIONS_WITH_ARGS: &[&str] = &[
    "--pre",          // Takes arbitrary command executed for each match
    "--pre-glob",     
    "--hostname-bin", // Executes command to get hostname
];

const UNSAFE_RIPGREP_OPTIONS: &[&str] = &[
    "--search-zip",   // Could search in zip bombs
    "-z",
];
```

These options are blocked because they could:
- Execute arbitrary commands (`--pre`, `--hostname-bin`)
- Search inside compressed files which could be zip bombs (`--search-zip`)

## 2. Library Integration

### File Search Module

Location: `codex-rs/file-search/`

The file-search module uses ripgrep's `ignore` crate (v0.4.23) for efficient file traversal:

```rust
use ignore::WalkBuilder;
```

Key features:
- Respects `.gitignore` files automatically
- Parallel directory traversal
- Efficient file filtering

From `codex-rs/file-search/README.md`:
> Uses https://crates.io/crates/ignore under the hood (which is what `ripgrep` uses) to traverse a directory (while honoring `.gitignore`, etc.)

The `ignore` crate is the core library that powers ripgrep's file discovery mechanism.

## 3. Prompt Guidance

### Standard Prompt
Location: `codex-rs/core/prompt.md`

```markdown
When searching for text or files, prefer using `rg` or `rg --files` respectively 
because `rg` is much faster than alternatives like `grep`. 
(If the `rg` command is not found, then use alternatives.)
```

### GPT-5 Prompt
Location: `codex-rs/core/gpt_5_codex_prompt.md`

Similar guidance is provided in the GPT-5 specific prompt, emphasizing ripgrep's performance advantages.

## Usage Patterns

### Common Use Cases

1. **Text Search**
   ```bash
   rg "pattern"                    # Search for pattern in current directory
   rg -n "pattern"                 # Include line numbers
   rg -i "pattern"                 # Case-insensitive search
   ```

2. **File Listing**
   ```bash
   rg --files                      # List all files (respecting .gitignore)
   rg --files --max-depth 2        # Limit directory depth
   ```

3. **Context Search**
   ```bash
   rg -C 3 "pattern"               # Show 3 lines of context
   rg -A 2 -B 2 "pattern"          # 2 lines after, 2 lines before
   ```

4. **Filtered Search**
   ```bash
   rg -g "*.rs" "pattern"          # Search only Rust files
   rg --glob "!vendor/*" "pattern" # Exclude vendor directory
   ```

## Performance Benefits

Ripgrep is preferred over traditional tools like `grep` because:

1. **Speed** - Uses parallelism and optimized algorithms
2. **Smart defaults** - Automatically ignores files in .gitignore
3. **Unicode support** - Handles UTF-8 efficiently
4. **Memory efficiency** - Uses memory maps when possible

## Integration Points

### Test Files
Multiple test files reference ripgrep functionality:
- `codex-rs/core/src/is_safe_command.rs` - Tests for safe/unsafe ripgrep invocations
- `codex-rs/execpolicy/tests/` - Various command execution tests

### Configuration
- Not bundled with Codex - expected to be available in the host environment
- Falls back to alternatives (`grep`) if not available

## Security Considerations

1. **Command Injection Prevention** - Only safe options are allowed
2. **Resource Protection** - Prevents searching in compressed files that could be malicious
3. **Execution Prevention** - Blocks options that execute external commands

## Notes

- Ripgrep is expected to be installed on the host system
- The `system_path` for ripgrep is left empty (`[]`) in the policy, indicating it should be found via PATH
- Per comment in `default.policy:139`: "Perhaps we need a way to indicate that we expect `rg` to be bundled with the host environment and we should be using that version"
- The integration focuses on safety while preserving most of ripgrep's useful functionality
- Unlike some tools, ripgrep is not bundled with Codex CLI - users must have it installed separately

## Related Files

- `codex-rs/execpolicy/src/default.policy` - Security policy configuration
- `codex-rs/core/src/is_safe_command.rs` - Safety validation logic
- `codex-rs/file-search/` - File search implementation using ignore crate
- `codex-rs/core/prompt.md` - Prompt instructions
- `codex-rs/core/gpt_5_codex_prompt.md` - GPT-5 specific prompt