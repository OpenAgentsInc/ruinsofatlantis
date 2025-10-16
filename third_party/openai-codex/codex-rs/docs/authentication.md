# Codex Authentication with OpenAI Platform and ChatGPT (Pro)

This document explains how Codex authenticates with OpenAI and ChatGPT, how it chooses which credentials and endpoints to use, how tokens are stored and refreshed, and exactly what headers and URLs are used at runtime.

The implementation lives primarily in the Rust workspace (`codex-rs`). Key modules and files are referenced inline.


## Overview

Codex supports two authentication modes:

- AuthMode::ApiKey: Use a standard OpenAI API key (e.g., via `OPENAI_API_KEY`) to call the OpenAI Platform API (`https://api.openai.com/v1`).
- AuthMode::ChatGPT: Sign in via the ChatGPT OAuth flow, then call ChatGPT’s backend (`https://chatgpt.com/backend-api/…`). This mode is preferred for individual plans (Free, Plus, Pro, Team) and unlocks ChatGPT‑specific functionality.

Selection between the two is automatic based on the tokens and plan type embedded in the id_token after login, but can be influenced by configuration. Details below.


## Where the logic lives

- Login and token storage: `codex-rs/login/src/`
  - OAuth flow + token persistence: `server.rs`
  - High‑level auth API and refresh: `lib.rs`
  - Token parsing and plan detection: `token_data.rs`
  - In‑process cache/manager: `auth_manager.rs`
- Model client and request wiring: `codex-rs/core/src/`
  - Provider registry, base URLs, and headers: `model_provider_info.rs`
  - Streaming requests (Responses API / Chat Completions): `client.rs` and `chat_completions.rs`
  - Configuration (including ChatGPT base URL, auth preference): `config.rs`, `config_profile.rs`
- ChatGPT helpers (task fetch, etc.): `codex-rs/chatgpt/src/`


## Auth modes and how Codex chooses

Codex loads auth from `$CODEX_HOME/auth.json` or the environment. The decision logic is in `codex-rs/login/src/lib.rs::load_auth()` and `codex-rs/login/src/token_data.rs::TokenData::should_use_api_key()`.

- If `auth.json` contains an API key and the plan type indicates a “metered”/enterprise style (Business, Enterprise, Edu), Codex uses AuthMode::ApiKey.
- If `auth.json` contains an API key but the plan is Free, Plus, Pro, or Team, Codex prefers AuthMode::ChatGPT and ignores the API key.
- If `auth.json` is missing, Codex falls back to `OPENAI_API_KEY` from the environment (if present), in which case it uses AuthMode::ApiKey.
- If you set `preferred_auth_method` in config to `api_key`, that forces ApiKey mode where possible; otherwise `chatgpt` is the default preference.
- If the email in the token ends with `@openai.com`, Codex prefers ChatGPT unless forced to ApiKey by config.

Plan detection comes from the `id_token` JWT claim `https://api.openai.com/auth.chatgpt_plan_type`, parsed in `token_data.rs`. Known plans: `free`, `plus`, `pro`, `team`, `business`, `enterprise`, `edu`.


## OAuth login flow (ChatGPT)

The login flow is implemented in `codex-rs/login/src/server.rs` using a local HTTP server and PKCE. A condensed walkthrough:

1. Start local server and PKCE
   - `run_login_server(ServerOptions)` binds `127.0.0.1:{port}` (default 1455) and generates PKCE codes.
   - It builds an authorize URL to the issuer (default `https://auth.openai.com`) with:
     - `response_type=code`
     - `scope=openid profile email offline_access`
     - `code_challenge` + `code_challenge_method=S256`
     - `id_token_add_organizations=true`
     - `codex_cli_simplified_flow=true`
     - `state={random}` (anti‑CSRF)
     - `redirect_uri=http://localhost:{port}/auth/callback`
   - The URL opens in a browser or is printed for manual navigation.

2. Authorization code → tokens
   - The browser redirects to `/auth/callback?code=…&state=…`.
   - Codex verifies `state`, then POSTs to `{issuer}/oauth/token` (form‑encoded) with:
     - `grant_type=authorization_code`
     - `code`, `redirect_uri`, `client_id`, `code_verifier`
   - On success, the response includes `id_token` (JWT), `access_token` (JWT), and `refresh_token`.

3. Token exchange to mint an OpenAI API key (optional)
   - Using the `id_token`, Codex performs an OAuth 2.0 Token Exchange to request an API key:
     - POST `{issuer}/oauth/token` (form‑encoded) with
       `grant_type=urn:ietf:params:oauth:grant-type:token-exchange`,
       `requested_token=openai-api-key`,
       `subject_token={id_token}`,
       `subject_token_type=urn:ietf:params:oauth:token-type:id_token`.
   - If successful, the body contains `access_token` which is stored as `OPENAI_API_KEY` in `auth.json`.

4. Persist tokens to `$CODEX_HOME/auth.json`
   - `persist_tokens_async()` writes:
     ```jsonc
     {
       "OPENAI_API_KEY": "sk-…" | null,
       "tokens": {
         "id_token": "<raw JWT>",        // serialized via wrapper with parsed info available
         "access_token": "<JWT>",        // used as bearer for ChatGPT backend
         "refresh_token": "<opaque>",
         "account_id": "<chatgpt_account_id>" // extracted from id_token claims if present
       },
       "last_refresh": "<timestamp>"
     }
     ```
   - File permissions on Unix are `0600`.

5. Success page and plan info
   - The login server redirects to `/success` which shows a confirmation page (`assets/success.html`).
   - Plan and account metadata used on that page and elsewhere are extracted from JWT claims at `https://api.openai.com/auth`.


## Token refresh

Codex refreshes tokens both proactively and reactively:

- Proactive refresh: When accessing `CodexAuth::get_token_data()`, if `last_refresh` is older than 28 days, Codex POSTs JSON to `https://auth.openai.com/oauth/token` with `grant_type=refresh_token` and updates `auth.json` on success. See `try_refresh_token()` and `update_tokens()` in `login/src/lib.rs`.
- Reactive refresh on 401: If a model request returns HTTP 401 (`core/src/client.rs`), Codex asks `AuthManager` to refresh before retrying.


## Where and how tokens are used

Two major codepaths handle model requests depending on the provider wire API:

1. OpenAI Platform (Responses API or Chat Completions)
   - Base URL (default): `https://api.openai.com/v1` (overridable via provider config or `OPENAI_BASE_URL`).
   - `ModelProviderInfo::create_request_builder()` (`core/src/model_provider_info.rs`) attaches:
     - `Authorization: Bearer {token}` where:
       - In ApiKey mode: `{token}` is the API key.
       - In ChatGPT mode: `{token}` is the ChatGPT access_token JWT (rare in practice for platform; ChatGPT mode normally targets ChatGPT endpoints, see below).
     - Optional headers via env: `OpenAI-Organization`, `OpenAI-Project`.
   - Responses API requests also include headers:
     - `OpenAI-Beta: responses=experimental`
     - `originator: {codex header}`
     - `session_id: {uuid}`
     - `User-Agent: codex/{version}` (via `get_codex_user_agent`)

2. ChatGPT backend (preferred for Free/Plus/Pro/Team plans)
   - Default base URL: `https://chatgpt.com/backend-api/` (configurable via `chatgpt_base_url`).
   - For model requests, the provider swaps to `https://chatgpt.com/backend-api/codex` and appends `/responses` or `/chat/completions` depending on the wire API (`core/src/model_provider_info.rs`).
   - Additional header: `chatgpt-account-id: {account_id}` if present (extracted from the login id_token claims and persisted in `auth.json`).
   - Bearer token: The ChatGPT `access_token` (JWT), from `auth.json.tokens.access_token`.
   - ChatGPT helpers (`codex-rs/chatgpt`):
     - Example: `GET {chatgpt_base_url}/wham/tasks/{id}` with:
       - `Authorization: Bearer {access_token}`
       - `chatgpt-account-id: {account_id}`
       - `User-Agent: codex/{version}`

Note: When in ChatGPT mode, Codex makes a compatibility adjustment for the web search tool: it renames `web_search` to `web_search_preview` when building tool JSON for the Responses API (`core/src/client.rs`).


## The in‑process AuthManager

`AuthManager` (`login/src/auth_manager.rs`) provides a single source of truth for the current `CodexAuth`:

- Loads once from disk (or env) based on `preferred_auth_method` (default `ChatGPT`).
- Returns cloned `CodexAuth` to callers; supports `reload()` after login completes.
- Exposes `refresh_token()` which refreshes, updates `auth.json`, and reloads the cached auth.

This manager is created by the server-side components and passed into the model client (`core/src/codex.rs` → `ModelClient::new`) so all requests use up-to-date credentials.


## CLI entry points (summary)

- `codex login` (ChatGPT): `codex-rs/cli/src/login.rs::login_with_chatgpt` starts the local login server and blocks until completion.
- `codex login --api-key <key>`: Writes a simple `auth.json` with `OPENAI_API_KEY`.
- `codex login --status`: Reports current mode and where the API key was sourced from.
- `codex logout`: Removes `$CODEX_HOME/auth.json`.


## Storage and security notes

- `auth.json` lives under `$CODEX_HOME` (default `~/.codex`).
- On Unix, Codex writes `auth.json` with mode `0600`.
- No browser automation or cookie scraping is used; the flow is standard OIDC+PKCE against `https://auth.openai.com`.


## Decision matrix highlights

- If you log in via ChatGPT and Codex can also mint an API key via token exchange, both will be stored. Codex still prefers ChatGPT mode for Free/Plus/Pro/Team because those plans are designed to use ChatGPT.
- If your plan is Business/Enterprise/Edu (or unknown), Codex will use the API key when available.
- If `auth.json` does not exist and `OPENAI_API_KEY` is set in your environment, Codex uses ApiKey mode.
- You can override default preference using `preferred_auth_method` in config.


## Relevant code references

- Login flow, token exchange, persistence:
  - `codex-rs/login/src/server.rs`
- Core auth types and refresh logic:
  - `codex-rs/login/src/lib.rs`
  - `codex-rs/login/src/auth_manager.rs`
  - `codex-rs/login/src/token_data.rs`
- Provider wiring and request headers:
  - `codex-rs/core/src/model_provider_info.rs`
  - `codex-rs/core/src/client.rs`
- ChatGPT helper client:
  - `codex-rs/chatgpt/src/`
- CLI entry points:
  - `codex-rs/cli/src/login.rs`

