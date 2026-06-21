# PRD: Claude-Specific Context Compression via MCP

**Owner:** Claude Code  
**Status:** Design approved  
**Goal:** Reduce Claude Code agent token consumption by 40–60% on live-zone I/O while improving agent accuracy and reducing workload.

---

## Problem Statement

Claude Code agents operate in tight loops: they call tools, get verbose outputs (stack traces, logs, API responses, retry metadata), and iterate. These tool outputs are often 90% noise (timestamps, backoff durations, retry counts) and 10% signal (actual error or result). This verbosity:

- **Wastes tokens** — Each turn consumes 40–60% more tokens than necessary on already-seen context
- **Hurts agent accuracy** — Agent gets confused by noise, tries the same approach twice, loses its reasoning chain
- **Increases workload** — More tokens = more compute, longer latency, higher API costs

Today, Claude Code users have no way to compress this live-zone I/O without modifying their agent code or running a separate proxy.

---

## Solution

Ship a Claude-specific **context compression MCP server** that:

1. **Compresses only the live zone** — This turn's user message + latest tool/function outputs (shell, file ops, API fetch). Preserves full conversation history and system prompt (agent needs these to reason and self-correct).

2. **Routes by content type** — Detects JSON tool outputs, code (stack traces, diffs), and prose (logs, error messages), compressing each via the right algorithm (SmartCrusher for JSON, CodeCompressor for code, Kompress-base for text).

3. **Preserves signal, compresses noise** — Uses hand-coded signal maps per tool type to preserve error messages and results, aggressively compress timestamps, retry counts, and metadata.

4. **Enables reversal (CCR)** — Stores originals locally; if Claude's agent needs the full output to debug, it calls `headroom_retrieve(output_id)` to recover it.

5. **Rolls out in two phases** — Phase 1 (manual, measured): agent explicitly calls compression tools. Phase 2 (semi-auto): Claude Code hook auto-compresses post-response.

6. **Enforces safety invariants** — Never touches authentication headers, tool definitions, function signatures, or sensitive error metadata.

---

## User Stories

1. As a Claude Code user running a long-running agent task, I want tool outputs compressed so that I consume fewer tokens per turn.

2. As a Claude Code user, I want compression to be opt-in initially so that I can measure its impact before rolling it out everywhere.

3. As a Claude Code user, I want my agent to still see its full conversation history so that it can reference past attempts and avoid retry loops.

4. As a Claude Code user, I want to be able to retrieve the original tool output if my agent needs to debug so that I'm not losing information.

5. As a Claude Code user, I want compression to be transparent (happen in the background) once I trust it so that my agent doesn't need to think about it.

6. As a Claude Code user, I want shell command outputs compressed more aggressively than API responses so that each tool gets the right compression level.

7. As a Claude Code user, I want error messages preserved in full so that my agent doesn't miss diagnostic info.

8. As a Claude Code user, I want timestamps, retry counts, and backoff durations removed from tool outputs so that noise doesn't inflate token usage.

9. As a Claude Code user, I want file operation results (paths, permissions, file sizes) preserved so that my agent understands what files it's working with.

10. As a Claude Code user, I want API response bodies preserved and headers/metadata compressed so that I keep the data my agent needs.

11. As a Claude Code user, I want to measure token reduction per turn so that I know compression is working.

12. As a Claude Code user, I want to measure agent accuracy (retry loops, first-try success) so that I know compression isn't hurting my agent's reasoning.

13. As a Claude Code user, I want to measure workload reduction so that I understand the cost savings.

14. As a Claude Code user, I want my authentication headers handled securely so that credentials are never logged or compressed.

15. As a Claude Code user, I want tool definitions (the schema of what tools are available) to never be compressed so that my agent always knows what it can do.

16. As a Claude Code developer building a Claude Code plugin, I want to reuse the compression MCP server without modifying Claude Code's core so that the feature is maintainable and independent.

17. As a Headroom maintainer, I want compression rules to be declarative (signal maps, not hard-coded logic) so that new tool types can be added without recompiling.

18. As a Headroom maintainer, I want the CCR (reversible compression) backend to be persistent so that users can retrieve originals across sessions.

19. As a Claude Code user, I want to know which specific outputs were compressed so that I can audit what my agent is seeing.

20. As a Claude Code user, I want to opt out of compression for specific sensitive tools so that I have granular control.

---

## Implementation Decisions

### 1. Integration Point: MCP Server

**Decision:** Compression lives as a standalone MCP server, not embedded in Claude Code or as a middleware hook.

**Rationale:**
- Zero modification to Claude Code internals — feature is isolated and testable
- Headroom already has MCP tools (`headroom_compress`, `headroom_retrieve`, `headroom_stats`) — we can adapt them
- Agent can explicitly call compression (Phase 1) and later auto-compress via hook (Phase 2) without coupling to Claude's architecture
- Supports any LLM client that speaks MCP (Claude Code, Cursor, Copilot) without duplication

**Interface:**
- `headroom_compress(tool_name: string, raw_output: string) → compressed_output: string`
- `headroom_retrieve(output_id: string) → original_output: string`
- `headroom_stats(session_id: string) → {tokens_saved, accuracy_delta, workload_reduction}`

---

### 2. Scope: Live-Zone-Only Compression

**Decision:** Compress only the current turn's user message and latest tool outputs. Never touch system prompt, previous turns, tool definitions, or old outputs.

**Rationale:**
- Maximizes token savings (tool outputs are the noisiest part) without risking accuracy
- Preserves prefix cache hits across turns (Claude's KV cache key depends on immutable context)
- Agent retains full reasoning chain — can self-correct, debug, recognize retry loops
- Matches Headroom's production pattern (Phase B: "live-zone-only compression")
- Simpler safety model — append-only changes, easier to audit

**Invariant:** If a byte in the system prompt, tool definitions, or previous turns wasn't intended for modification, it arrives at the LLM API byte-equal to how it arrived at the proxy.

---

### 3. Content Routing & Compression Algorithms

**Decision:** Three-algorithm router, content-aware:
- **SmartCrusher** — JSON tool outputs (API responses, structured data): drop non-critical keys, collapse nested objects, abbreviate values
- **CodeCompressor** — Code in tool output (stack traces, diffs, snippets): parse AST, drop comments, abbreviate variable names, normalize formatting
- **Kompress-base** — Text outputs (logs, error messages, prose): learned text compression via Headroom's HuggingFace model

**ContentRouter** auto-detects content type and selects the right compressor.

**Rationale:**
- Specialized compressors beat generic ones (e.g., JSON doesn't benefit from code compression, and vice versa)
- Headroom's existing algorithms are proven in production
- Field-tested detection logic reduces false positives

---

### 4. Signal Tagging & Accuracy Improvement

**Decision:** Each compressor has a hand-coded **signal map** per tool type that labels output fields/sections as signal (preserve) or noise (compress):

- **Shell tool:** signal = `stdout`, `exit_code`, `error_message`; noise = `elapsed_ms`, `retries`, `backoff_duration`
- **File ops tool:** signal = `paths`, `file_contents`, `permissions`; noise = `mtime`, `size_bytes` (if not relevant to task)
- **Fetch tool:** signal = `response_body`, `status_code`; noise = `headers`, `elapsed_ms`, `dns_time`

Compressor preserves signal paths, aggressively compresses noise paths.

**Rationale:**
- Improves accuracy by keeping diagnostic info agent needs to reason about
- Reduces hallucination (agent doesn't lose error messages)
- Improves retry-loop detection (agent can see it tried the same thing before)

---

### 5. Reversible Compression (CCR) & Retrieval

**Decision:** Original outputs are stored in a persistent CCR backend (local file-based or cloud). Each compressed output is tagged with a `ccr_id`. If agent needs original, it calls `headroom_retrieve(ccr_id)`.

**Rationale:**
- Agent can ask for full output if compression hides something critical
- No information is permanently lost — safety net for edge cases
- Headroom's existing CCR pattern is proven

---

### 6. Initial Tool Coverage: Shell, File Ops, Fetch

**Decision:** Phase 1 targets these three tool types only:
1. Shell/Bash (highest noise, highest savings)
2. File operations (structured, compressible)
3. HTTP fetch (JSON response bodies)

Additional tools (database, LLM-as-tool, custom) can be added post-launch.

**Rationale:**
- These three account for ~70% of typical agent I/O
- Each has clear signal/noise boundaries
- Easier to define rules and test

---

### 7. Rollout in Two Phases

**Phase 1 — Manual Compression (Week 1–2):**
- Add `headroom_compress` and `headroom_retrieve` tools to Claude's system prompt
- Agent sees verbose output, decides to compress, calls tool explicitly
- Hook instrumentation logs: input tokens, output tokens, retry count, accuracy delta
- Measure for 1–2 weeks; validate metrics before Phase 2

**Phase 2 — Semi-Auto Compression (Week 3+):**
- Add Claude Code hook that auto-calls `headroom_compress` on shell/file/fetch outputs post-response
- Agent doesn't think about compression; it's transparent
- Retrieval remains explicit: agent calls `headroom_retrieve` if needed
- Shipping criteria: tokens down 40–60%, accuracy metrics neutral or improved, no retry-loop regression

---

### 8. Safety Invariants (Never Compress)

**Decision:** Four classes of content are always passthrough-only:

1. **Authentication headers** — `Authorization`, `X-API-Key`, bearer tokens, session IDs: byte-faithful, never logged
2. **Error messages with sensitive info** — Paths, endpoints, version strings, credential hints: preserved in full
3. **Tool definitions** — System prompt's `tools` array: never modified, always normalized (sorted keys)
4. **Function signatures** — In stack traces and code snippets: preserved, only arguments/bodies compressed

**Implementation:**
- ContentRouter checks for these patterns before routing to a compressor
- If detected, pass through with marker `"_headroom_passthrough": true`
- Tests verify no compression occurs for these patterns

---

### 9. Metrics & Instrumentation

**Decision:** Three metrics tracked post-each-turn:

1. **Token delta** — `(tokens_before - tokens_after) / tokens_before` as percentage. Target: 40–60%
2. **Accuracy delta** — Count:
   - Retry loops (same tool called twice in a row with same args)
   - Self-corrections (agent fixes its own error)
   - Hallucinations (agent claims something not in context)
   - First-try success (task completed without retry)
3. **Workload reduction** — Proxy metric: `tokens_saved * cost_per_token` = cost reduction

**Instrumentation points:**
- MCP server logs on each compress/retrieve call
- Claude Code hook logs metrics to `.claude/metrics.jsonl` post-turn
- E2E test comparison: same prompts, compressed vs. uncompressed, measure delta

---

### 10. Configuration & Control

**Decision:** Phase 1 is manual (agent decides), Phase 2 allows user overrides via settings:

- `compression.enabled` — Global on/off (default: off in Phase 1, on in Phase 2)
- `compression.tools` — Per-tool enable/disable (e.g., `{"shell": true, "file": true, "fetch": false}`)
- `compression.level` — Compression aggressiveness (low/medium/high; controls signal map thresholds)
- `compression.auth_safe_mode` — Extra paranoia: never compress any output from tools that touch auth (default: true)

---

## Testing Decisions

### What Makes a Good Test

A good test verifies **external behavior**, not implementation:
- Test the MCP interface (input → output), not internal router logic
- Test the signal map rules (given noisy shell output, is the error preserved?), not the compression algorithm itself
- Test safety invariants (auth headers never appear in compressed output), not compressor internals
- Test accuracy metrics (does retry-loop count stay the same?), not token math

### Modules to Test

1. **ContentRouter** — Detects JSON vs. code vs. text correctly; no misrouting
2. **Signal maps** — Per-tool-type (shell, file, fetch), verify signal fields preserved and noise dropped
3. **MCP server interface** — `compress(tool, output)` and `retrieve(id)` round-trip correctly
4. **CCR storage/retrieval** — Originals persisted and recovered; no data loss
5. **Safety invariants** — Auth headers, errors, tool defs never touched; passthrough marker applied
6. **Metrics** — Token delta, retry count, hallucination detection work correctly
7. **Phase 1 vs Phase 2** — Manual tool calls (Phase 1) and auto-compression hook (Phase 2) produce same output

### Prior Art in Codebase

Headroom's own test structure:
- `crates/headroom-core/tests/` — Content routing tests, signal map tests
- `crates/headroom-proxy/tests/` — MCP interface tests, CCR retrieval tests
- `benchmarks/` — Token delta benchmarking (use as template for accuracy metrics)

Reuse existing Headroom test patterns where possible.

---

## Out of Scope

- **Compression for outputs beyond shell/file/fetch** — Database results, LLM-as-tool, custom tools — deferred to Phase 3
- **Multi-agent compression** — Cross-agent memory, shared CCR store — future work
- **Output compression** — Compressing what Claude writes back (Headroom does this; out of scope here)
- **Streaming** — Compress streamed tool outputs in real-time — defer to later release
- **Custom compression rules per user** — Declarative signal maps only; no per-user ML models
- **Integration with other LLM frameworks** — LangChain, LlamaIndex, etc. — focus on Claude Code first
- **Persistent metrics storage** — Metrics stored locally; no cloud dashboard yet

---

## Further Notes

### Why This Matters for Claude Code

Claude Code agents are workhorses. They:
- Run in tight loops (many turns per session)
- Call the same tools repeatedly (shell, file, fetch)
- Need to remember what they tried (to avoid retry loops)

Today, a 10-turn debugging session might consume 50K tokens, of which 30K is repetitive tool noise. With compression:
- Same session = 20K tokens (40% savings)
- Agent sees full reasoning chain (old turns untouched) — better self-correction
- Workload down 40% — latency, cost, compute all improve

### Why Live-Zone-Only Works

Headroom's biggest insight: the **hot zone** (system prompt, tool defs, old conversation turns) needs to stay pristine for accuracy. Only compress the **live zone** (this turn's I/O). This:
- Maximizes savings (tool outputs are noisiest)
- Maximizes accuracy (agent keeps its reasoning)
- Maximizes cache efficiency (prefix never changes, cache always hits)

### Migration Path

If this succeeds (metrics look good, no accuracy drift):
1. Phase 2: Ship Phase 2 semi-auto compression
2. Expand tool coverage: database, custom tools
3. Integrate streaming compression for real-time tools
4. Add cloud metrics dashboard for ops visibility

### Relationship to Headroom

This is **not** a fork of Headroom's full compression suite. It's a **focused extraction** of Headroom's core patterns (live-zone + signal tagging + CCR) optimized for Claude Code. Keeps Headroom as the source of truth; leverages its proven algorithms and safety invariants.

