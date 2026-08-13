# Architecture: Headroom-Inspired Agentic Compression Framework

## Overview

This MCP server provides context compression for Claude Code agents, reducing token consumption by 40-60% through content-aware compression algorithms while preserving signal (error messages, function signatures, results) and removing noise (timestamps, retry counts, metadata).

## Core Components

### 1. MCP Server (main.rs)

Entry point for the MCP server. Implements the Model Context Protocol interface and exposes three main tools:

- **`headroom_compress`**: Compresses tool output based on content type detection
- **`headroom_retrieve`**: Retrieves original output by ID for debugging
- **`headroom_stats`**: Returns compression metrics

### 2. ContentRouter (router.rs)

Routes compression requests to appropriate algorithms based on content type detection:

```
Input → ContentType::detect() → Route to Compressor → Output
```

**Supported content types:**
- `ContentType::Json` → SmartCrusher
- `ContentType::Code` → CodeCompressor  
- `ContentType::Text` → KompressBase
- `ContentType::Unknown` → Text (fallback)

**Extensibility:** New compressors can be registered via `router.register()`.

### 3. Compressor Trait (compressors/mod.rs)

All compressors implement the `Compressor` trait:

```rust
pub trait Compressor: Send + Sync {
    fn compress(&self, content: &str) -> Result<(String, f64), MpcError>;
    fn name(&self) -> &str;
}
```

Returns compressed content and compression ratio (original_size / compressed_size).

### 4. Concrete Compressors

#### CacheAligner (compressors/cache_aligner.rs)
**Target:** Dynamic non-deterministic tokens (timestamps, hex memory pointers, ephemeral UUIDs, millisecond latencies, PIDs)
- Normalizes high-entropy tokens to canonical placeholders (`<TIMESTAMP>`, `<HEX_ADDR>`, `<UUID>`, `<DURATION>`, `<PID>`)
- Maximizes provider-side KV prompt caching hit rates (>80%) and reduces TTFT latency.

#### SmartCrusher 2.0 (compressors/smart_crusher.rs)
**Target:** JSON tool outputs (API responses, structured logs)
- **Schema-guided Tabular Projection:** Converts homogeneous JSON object arrays to compact tabular columns (`#cols:k1|k2...`), cutting token usage by 65–85%.
- **Anomaly/Error Preservation:** Preserves error and exception records in full detail, condensing routine 200/OK records.
- **Deep Noise Pruning:** Strips telemetry, timestamps, latency metrics, and empty structures.

#### CodeCompressor 2.0 (compressors/code_compressor.rs)
**Target:** Code-related outputs (stack traces, diffs, function signatures)
- **AST Skeletonization:** Retains function signatures, docstrings, and type definitions while folding large bodies into concise stub annotations.
- **Unified Diff Compaction:** Prunes redundant unchanged context lines around hunks, preserving `@@` headers and modified delta lines (`+`/`-`).
- **Stack Trace De-noising:** Collapses repetitive framework/runtime frames (e.g. `tokio`, `std::panicking`) while strictly preserving application frames and root causes.

#### KompressBase 2.0 (compressors/kompress_base.rs)
**Target:** General text/prose (logs, command output)
- Multi-rubric filtering for error & diagnostic preservation.
- Collapses progress bar spam and repetitive polling loops.

### 5. Signal Maps (signal_maps/)

Per-tool-type rules for preserving critical information:

**TODO (Issue #5):**
- `shell.rs`: Preserve exit codes, errors, file paths
- `file_ops.rs`: Preserve file paths, permissions, sizes
- `fetch.rs`: Preserve response status, body; compress headers, timing

### 6. Safety Invariants (safety.rs)

Enforces critical information protection:

- Authentication headers (Authorization, api_key, token, secret)
- Error messages (Error:, Exception:, Fatal:, Panic:)
- Tool definitions
- Function signatures
- Sensitive metadata

### 7. CCR Backend (ccr.rs)

Reversible compression storage using UUIDs:

```
Original → store() → UUID → retrieve(UUID) → Original
```

Enables agents to debug by retrieving full outputs when needed.

### 8. Metrics (metrics.rs)

Instrumentation for measuring compression effectiveness:

- `total_tokens_saved`: Cumulative tokens compressed
- `compressions_count`: Total successful compressions
- `errors_count`: Compression failures
- `average_accuracy`: Quality score (0.0-1.0)

## Data Flow

### Compression Flow
```
1. Agent calls headroom_compress(tool, raw_output)
2. MCP server receives request
3. ContentRouter.compress() detects content type
4. Routes to appropriate compressor
5. Safety checks run (skip if critical info detected)
6. Original stored in CCR backend (get UUID)
7. Compressed output + UUID returned to agent
```

### Retrieval Flow
```
1. Agent calls headroom_retrieve(output_id)
2. MCP server looks up in CCR backend
3. Returns original output (byte-equal)
```

### Stats Flow
```
1. Agent calls headroom_stats()
2. MCP server returns current metrics snapshot
3. Includes token savings, accuracy, workload reduction
```

## Design Decisions

### 1. MCP Server (Not Inline Library)
**Why:** Isolated, testable, reusable across clients (Claude Code, Cursor, Copilot).

### 2. Live-Zone-Only Scope
**Why:** Preserve full conversation history for agent reasoning. Only compress this turn's tool outputs.

### 3. Three-Algorithm Router
**Why:** Different content requires different strategies:
- JSON: structural, use field-level rules
- Code: pattern-based, preserve sigs/line numbers
- Text: semantic, use language model

### 4. Signal Tagging (Per-Tool Rules)
**Why:** Hand-coded rules preserve accuracy better than generic algorithms for known tool types. Declarative format enables new tools without recompiling.

### 5. CCR Reversible Compression
**Why:** Agents need the option to retrieve full outputs for debugging. Stores original locally; no data loss.

### 6. Two-Phase Rollout
**Why:** Phase 1 (manual) allows measurement and validation before Phase 2 (auto-compression via hooks).

## Module Dependency Graph

```
main.rs
├── router.rs
│   └── compressors/mod.rs
│       ├── smart_crusher.rs
│       ├── code_compressor.rs
│       └── kompress_base.rs
├── ccr.rs
├── metrics.rs
└── safety.rs

mcp-types/lib.rs
├── ContentType
├── CompressRequest/Response
├── RetrieveRequest/Response
├── StatsRequest/Response
└── MpcError
```

## Testing Strategy

Each module includes unit tests:

- **CompressorTests:** Verify no-op behavior (Issue #1), then full compression (Issues #2-4)
- **RouterTests:** Content type detection, routing logic, extensibility
- **CCRTests:** Store/retrieve roundtrip, UUID generation
- **MetricsTests:** Accumulation, reset, snapshot generation
- **SafetyTests:** Critical info detection, skip conditions
- **IntegrationTests:** Full compress → retrieve → metrics flow

## Future Enhancements

### Phase 2: Semi-Auto Compression
- Claude Code hook auto-compresses post-response
- Agent doesn't call compress explicitly
- Transparent compression tuning

### Phase 2: Advanced Signal Maps
- ML-based signal detection (learn what's important)
- Per-agent signal profiles
- Compression policy per session

### Phase 3: Cloud Integration
- Distributed CCR backend (persistent storage)
- Cloud-based model inference (Kompress-base)
- Cross-session retrieval
