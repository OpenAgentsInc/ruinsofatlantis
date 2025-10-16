# Tools Sent to ChatGPT Backend

This document details all the tools that get sent to the ChatGPT backend when making API requests.

## Tool Assembly

The main function that assembles all tools is `get_openai_tools` in `codex-rs/core/src/openai_tools.rs:525`.

## Available Tools

### 1. Shell/Execution Tools

The type of shell tool depends on configuration:

#### a. **unified_exec** (Experimental)
Location: `codex-rs/core/src/openai_tools.rs:207`
- Name: `unified_exec`
- Description: Runs a command in a PTY with support for interactive sessions
- Parameters:
  - `input`: Command/arguments array or stdin input for existing session
  - `session_id`: Identifier for reusing interactive sessions
  - `timeout_ms`: Maximum wait time for output

#### b. **shell** (Default)
Location: `codex-rs/core/src/openai_tools.rs:173`
- Name: `shell`
- Description: Runs a shell command and returns its output
- Parameters:
  - `command`: Array of command strings to execute
  - `workdir`: Working directory for execution
  - `timeout_ms`: Command timeout in milliseconds

#### c. **shell with sandbox** (When approval_policy is OnRequest)
Location: `codex-rs/core/src/openai_tools.rs:254`
- Same as default shell but with additional parameters:
  - `with_escalated_permissions`: Request to run without sandbox restrictions
  - `justification`: Explanation for escalated permissions request

#### d. **local_shell**
- Type: `LocalShell`
- Special shell type for models that support it directly

#### e. **Streamable shell** (Two tools)
- `exec_command`: Execute commands with streaming output
- `write_stdin`: Write to stdin of running processes

### 2. **plan** Tool
Location: `codex-rs/core/src/plan_tool.rs`
- Name: `plan`
- Description: Helps plan complex tasks
- Enabled when: `include_plan_tool` is true
- Used for: Breaking down complex tasks into steps

### 3. **apply_patch** Tool
Location: `codex-rs/core/src/tool_apply_patch.rs`

Two variants based on configuration:

#### a. **Freeform variant**
- Name: `apply_patch_freeform`
- Format: Custom freeform format
- Used for: Applying code changes in a flexible format

#### b. **Function variant**  
- Name: `apply_patch`
- Format: JSON function calling
- Parameters:
  - `input`: The patch content to apply

### 4. **web_search** Tool
- Type: `WebSearch`
- Description: Search the web for information
- Note: Currently has API issues per comments
- Enabled when: `include_web_search_request` is true

### 5. **view_image** Tool
Location: `codex-rs/core/src/openai_tools.rs:303`
- Name: `view_image`
- Description: Attach a local image to the conversation context
- Parameters:
  - `path`: Local filesystem path to an image file
- Enabled when: `include_view_image_tool` is true

### 6. **MCP (Model Context Protocol) Tools**
Location: Dynamic, from MCP servers
- These are external tools provided by MCP servers
- Each MCP tool is converted to OpenAI function format
- Names are fully qualified (e.g., `server_name__tool_name`)
- Parameters depend on the specific MCP tool

## Tool Configuration

The tools included depend on the `ToolsConfig` which is built from:

```rust
pub(crate) struct ToolsConfigParams {
    model_family: &ModelFamily,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    include_plan_tool: bool,
    include_apply_patch_tool: bool,
    include_web_search_request: bool,
    use_streamable_shell_tool: bool,
    include_view_image_tool: bool,
    experimental_unified_exec_tool: bool,
}
```

## JSON Format Conversion

Tools are converted to Chat Completions format in two steps:

1. **Internal representation** (`OpenAiTool` enum) is serialized to Responses API format
2. **Chat Completions conversion** (`create_tools_json_for_chat_completions_api`):
   - Filters out non-function tools (Chat Completions only supports functions)
   - Wraps each tool in `{ "type": "function", "function": {...} }` structure

## Example Tool JSON

Here's what a shell tool looks like when sent:

```json
{
  "type": "function",
  "function": {
    "name": "shell",
    "description": "Runs a shell command and returns its output",
    "parameters": {
      "type": "object",
      "properties": {
        "command": {
          "type": "array",
          "items": {"type": "string"},
          "description": "The command to execute"
        },
        "workdir": {
          "type": "string",
          "description": "The working directory to execute the command in"
        },
        "timeout_ms": {
          "type": "number",
          "description": "The timeout for the command in milliseconds"
        }
      },
      "required": ["command"],
      "additionalProperties": false
    }
  }
}
```

## Tool Selection Logic

The actual tools sent depend on:

1. **Model capabilities**: Different models support different tool types
2. **Configuration**: User settings and flags
3. **Security policies**: Sandbox and approval requirements
4. **MCP servers**: Additional tools from connected MCP servers
5. **Experimental features**: Like `unified_exec` tool

## Important Notes

- Only function-type tools are sent to Chat Completions API (others filtered out)
- Tools are sorted deterministically (especially MCP tools) to maximize prompt cache hits
- Some tools like `web_search` and `local_shell` have special handling by the API
- The `strict` parameter is generally set to `false` for flexibility