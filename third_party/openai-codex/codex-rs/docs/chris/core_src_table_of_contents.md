# Core Source Code Table of Contents

This document provides an overview of what each file in `codex-rs/core/src/` does.

## Main Library Files

### `lib.rs`
Main library entry point that defines the public API and re-exports modules. Contains clippy lints to prevent accidental stdout/stderr writes in library code.

### `codex.rs`
High-level interface to the Codex system. Contains:
- `Codex` struct - main queue-based interface for submissions and events
- `Session` struct - context for an initialized model agent
- `TurnContext` - context for a single conversation turn
- Core event loop and task management

## Authentication & Configuration

### `auth.rs`
Authentication management for different providers (OpenAI API keys, ChatGPT tokens).

### `config.rs`
Configuration file parsing, validation, and management. Handles `~/.codex/config.toml`.

### `config_edit.rs`
Configuration editing functionality for updating config files.

### `config_profile.rs`
Profile-based configuration management (different settings per profile).

### `config_types.rs`
Type definitions for configuration structures.

## Communication & API

### `chat_completions.rs`
Implementation of Chat Completions API communication with model providers. Handles:
- Message formatting for different providers
- Streaming responses via Server-Sent Events (SSE)
- Retry logic and error handling
- Tool calls integration

### `client.rs`
HTTP client implementation for API requests to model providers.

### `client_common.rs`
Common types and utilities shared between different client implementations:
- `Prompt` struct
- `ResponseEvent` enum
- `ResponseStream` for handling streaming responses

### `default_client.rs`
Factory functions for creating configured HTTP clients with proper headers and timeouts.

## Model & Provider Management

### `model_family.rs`
Defines model families (GPT-4, Claude, etc.) and their capabilities.

### `model_provider_info.rs`
Information about different model providers (OpenAI, Anthropic, etc.) including:
- API endpoints
- Authentication methods
- Request formatting

### `openai_model_info.rs`
Specific model information for OpenAI models.

## Tools & Execution

### `openai_tools.rs`
Tool definitions and JSON schema generation for OpenAI-compatible APIs:
- Shell/execution tools
- File manipulation tools
- Custom tools and MCP tools
- JSON schema sanitization

### `exec.rs`
Command execution framework with security policies.

### `exec_command/`
Directory containing streamable command execution:
- `mod.rs` - Module exports
- `exec_command_params.rs` - Parameter structures
- `exec_command_session.rs` - Session management
- `responses_api.rs` - API response formatting
- `session_id.rs` - Session ID management
- `session_manager.rs` - Session lifecycle management

### `unified_exec/`
Unified execution system:
- `mod.rs` - Main implementation
- `errors.rs` - Error types

### `shell.rs`
Shell detection and command formatting for different shells (bash, zsh, PowerShell).

### `spawn.rs`
Process spawning utilities with platform-specific handling.

## Security & Sandboxing

### `is_safe_command.rs`
Command safety validation. Determines which commands can be executed without approval:
- Allowlist of safe commands
- Dangerous option detection (e.g., ripgrep's `--pre`)
- Command argument validation

### `seatbelt.rs`
macOS Seatbelt sandbox integration using `/usr/bin/sandbox-exec`.

### `landlock.rs`
Linux Landlock LSM integration for filesystem access control.

### `safety.rs`
General safety utilities and policies.

## File Operations

### `apply_patch.rs`
Patch application functionality for code modifications.

### `tool_apply_patch.rs`
Tool definition for the apply_patch functionality.

### `tool_apply_patch.lark`
Grammar definition for patch parsing (Lark parser format).

## MCP (Model Context Protocol)

### `mcp_connection_manager.rs`
Manages connections to MCP servers and tool registration.

### `mcp_tool_call.rs`
Handles MCP tool call execution and response processing.

## History & State Management

### `conversation_history.rs`
Manages conversation history persistence and retrieval.

### `conversation_manager.rs`
High-level conversation management and coordination.

### `codex_conversation.rs`
Core conversation state and operations.

### `message_history.rs`
Message-level history tracking and management.

### `turn_diff_tracker.rs`
Tracks changes between conversation turns.

### `internal_storage.rs`
Internal storage mechanisms for persistent state.

## User Interface & Context

### `environment_context.rs`
Environment context information passed to models:
- Working directory
- Git repository info
- Sandbox policies
- Network access status

### `user_instructions.rs`
User instruction parsing and management.

### `user_notification.rs`
User notification system for important events.

### `custom_prompts.rs`
Custom prompt management and templating.

## Parsing & Processing

### `parse_command.rs`
Command line parsing utilities.

### `truncate.rs`
Text truncation utilities for managing context length.

### `event_mapping.rs`
Maps between internal events and protocol events.

## Utilities

### `util.rs`
General utility functions including backoff algorithms.

### `terminal.rs`
Terminal interaction utilities.

### `token_data.rs`
Token usage tracking and management.

### `flags.rs`
Feature flags and configuration flags.

### `git_info.rs`
Git repository information extraction.

### `project_doc.rs`
Project documentation extraction and processing.

## Review & Quality

### `review_format.rs`
Code review formatting and presentation.

### `rollout/`
Directory for rollout and deployment tracking:
- `mod.rs` - Module exports
- `list.rs` - Rollout lists
- `policy.rs` - Rollout policies
- `recorder.rs` - Event recording
- `tests.rs` - Test utilities

## Error Handling

### `error.rs`
Error types and error handling utilities for the core library.

## Execution Environment

### `exec_env.rs`
Execution environment setup and management.

### `bash.rs`
Bash-specific execution utilities.

## Support Files

### `seatbelt_base_policy.sbpl`
Base Seatbelt policy file for macOS sandboxing.

### `codex/`
Subdirectory containing:
- `compact.rs` - Conversation compaction utilities

## Architecture Overview

The core library is organized around several key concepts:

1. **Session Management** - `codex.rs` orchestrates everything
2. **Model Communication** - `chat_completions.rs` handles API calls
3. **Tool System** - `openai_tools.rs` defines available tools
4. **Security** - Multiple files handle sandboxing and safety
5. **State Management** - Various files manage conversation and execution state
6. **Configuration** - Multiple config-related files handle settings

The library is designed to be used by various frontends (TUI, CLI, etc.) while providing a consistent interface for model interaction and tool execution.