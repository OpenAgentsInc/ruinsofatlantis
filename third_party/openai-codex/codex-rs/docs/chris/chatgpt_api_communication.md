# ChatGPT API Communication Details

This document explains how prompts and tools are sent to the ChatGPT backend in the Codex codebase.

## Overview

The Codex CLI communicates with the ChatGPT backend using the Chat Completions API format. The main implementation is in `codex-rs/core/src/chat_completions.rs`.

## Request Flow

### 1. Entry Point: `stream_chat_completions`

Location: `codex-rs/core/src/chat_completions.rs:32`

This function orchestrates the entire process of sending prompts and tools to the ChatGPT API:

```rust
pub(crate) async fn stream_chat_completions(
    prompt: &Prompt,
    model_family: &ModelFamily,
    client: &reqwest::Client,
    provider: &ModelProviderInfo,
) -> Result<ResponseStream>
```

### 2. Message Construction

The function builds a messages array following the ChatGPT format:

#### System Message (Lines 41-42)
```rust
let full_instructions = prompt.get_full_instructions(model_family);
messages.push(json!({"role": "system", "content": full_instructions}));
```

#### Conversation History (Lines 44-268)
The function processes various types of conversation items:

- **User/Assistant Messages**: Standard text messages with role and content
- **Function Calls**: Assistant messages with tool_calls array
- **Local Shell Calls**: Special type of tool call for shell commands
- **Tool Outputs**: Responses from function/tool calls with role "tool"
- **Custom Tool Calls**: User-defined tools
- **Reasoning**: Attached to assistant messages when applicable

##### Reasoning Attachment Logic (Lines 51-134)
- Reasoning blocks are only attached if the conversation doesn't end with a user message
- They're mapped to adjacent assistant messages after the last user message
- Reasoning is added as a special "reasoning" field in the message JSON

### 3. Tools Configuration

Location: `codex-rs/core/src/openai_tools.rs`

#### Tool Types
The system supports multiple tool types:
- **Function Tools**: Standard OpenAI function calling
- **Local Shell**: Execute shell commands
- **Web Search**: Search the web (though commented as having API issues)
- **Freeform Tools**: Custom tool definitions
- **Plan Tool**: For planning complex tasks
- **Apply Patch Tool**: For code modifications

#### Tool JSON Creation (Line 270)
```rust
let tools_json = create_tools_json_for_chat_completions_api(&prompt.tools)?;
```

This function (`openai_tools.rs:667`) converts internal tool representations to Chat Completions format:
1. First creates Responses API format
2. Filters for function-type tools
3. Rewraps each tool in the Chat Completions structure:
   ```json
   {
     "type": "function",
     "function": { /* tool definition */ }
   }
   ```

### 4. Payload Construction

Lines 271-276 build the complete JSON payload:
```rust
let payload = json!({
    "model": model_family.slug,      // e.g., "gpt-4", "gpt-3.5-turbo"
    "messages": messages,            // Array of conversation messages
    "stream": true,                  // Always streaming responses
    "tools": tools_json,            // Array of available tools
});
```

### 5. Debug Logging

Lines 278-282 log the complete request for debugging:
```rust
debug!(
    "POST to {}: {}",
    provider.get_full_url(&None),
    serde_json::to_string_pretty(&payload).unwrap_or_default()
);
```

### 6. HTTP Request

#### URL Configuration
Location: `codex-rs/core/src/model_provider_info.rs:146`

The URL depends on the authentication mode:
- ChatGPT auth: `https://chatgpt.com/backend-api/codex`
- API key auth: `https://api.openai.com/v1`

#### Request Building
Lines 289-295 create and send the actual HTTP POST request:
```rust
let req_builder = provider.create_request_builder(client, &None).await?;

let res = req_builder
    .header(reqwest::header::ACCEPT, "text/event-stream")
    .json(&payload)                 // Attach JSON payload
    .send()
    .await;
```

The `create_request_builder` function (`model_provider_info.rs:126`):
1. Creates a POST request to the appropriate URL
2. Adds authentication (Bearer token for API key or ChatGPT token)
3. Applies any additional HTTP headers

### 7. Response Handling

#### Success Case (Lines 298-306)
- Creates an async channel for streaming events
- Spawns a task to process Server-Sent Events (SSE)
- Returns a ResponseStream for the caller

#### Error Handling with Retries (Lines 308-338)
- Implements exponential backoff for retryable errors
- Respects Retry-After headers from the server
- Maximum retry attempts configurable per provider

### 8. SSE Processing

Function: `process_chat_sse` (Lines 344+)
- Processes the streaming response from ChatGPT
- Converts SSE events to internal ResponseEvent format
- Handles idle timeouts for slow streams

## Authentication

### ChatGPT Authentication
Location: `codex-rs/chatgpt/src/chatgpt_client.rs`

For ChatGPT-authenticated requests:
- Uses access token from ChatGPT login
- Includes `chatgpt-account-id` header
- Bearer token authentication

### API Key Authentication
Standard OpenAI API key in Authorization header as Bearer token

## Example Payload

Here's what actually gets sent to the ChatGPT backend:

```json
{
  "model": "gpt-4",
  "messages": [
    {
      "role": "system",
      "content": "You are a helpful coding assistant..."
    },
    {
      "role": "user",
      "content": "Write a Python function to calculate factorial"
    },
    {
      "role": "assistant",
      "content": null,
      "tool_calls": [
        {
          "id": "call_abc123",
          "type": "function",
          "function": {
            "name": "write_file",
            "arguments": "{\"path\": \"factorial.py\", \"content\": \"...\"}"
          }
        }
      ]
    },
    {
      "role": "tool",
      "tool_call_id": "call_abc123",
      "content": "File written successfully"
    }
  ],
  "stream": true,
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "write_file",
        "description": "Write content to a file",
        "parameters": {
          "type": "object",
          "properties": {
            "path": {"type": "string"},
            "content": {"type": "string"}
          },
          "required": ["path", "content"]
        }
      }
    }
  ]
}
```

## Key Implementation Details

1. **Always Streaming**: The `stream` parameter is always set to true
2. **Reasoning Support**: Special handling for reasoning blocks that get attached to assistant messages
3. **Tool Flexibility**: Supports multiple tool types including custom/freeform tools
4. **Retry Logic**: Built-in retry mechanism with exponential backoff
5. **Provider Abstraction**: The same code works with different providers (OpenAI, ChatGPT, etc.)

## Related Files

- `codex-rs/core/src/chat_completions.rs`: Main implementation
- `codex-rs/core/src/openai_tools.rs`: Tool definitions and JSON construction
- `codex-rs/core/src/model_provider_info.rs`: Provider configuration and request building
- `codex-rs/chatgpt/src/chatgpt_client.rs`: ChatGPT-specific authentication
- `codex-rs/core/src/client_common.rs`: Common types (Prompt, ResponseEvent, etc.)