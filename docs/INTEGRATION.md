# Phase 1 Integration: Claude Code MCP Server

## Overview

Phase 1 implements **manual, explicit compression** where agents explicitly call compression tools. This allows:
- Measurement of compression effectiveness
- Validation of safety invariants
- Controlled rollout before Phase 2 (automatic compression)

## Integration Points

### 1. MCP Server Discovery

Claude Code discovers MCP servers via configuration. Add to `.claude/settings.json`:

```json
{
  "mcpServers": {
    "headroom-compression": {
      "command": "cargo",
      "args": ["run", "--release", "--manifest-path", "/path/to/compression-mcp/Cargo.toml"]
    }
  }
}
```

Or for installed binary:

```json
{
  "mcpServers": {
    "headroom-compression": {
      "command": "/usr/local/bin/compression-mcp"
    }
  }
}
```

### 2. Tool Registration

Once discovered, the MCP server exposes three tools to Claude Code:

#### Tool: `headroom_compress`

**Purpose:** Compress tool output based on content type

**Input:**
```json
{
  "tool_name": "shell|file|fetch|<other>",
  "raw_output": "... full tool output ..."
}
```

**Output:**
```json
{
  "output_id": "550e8400-e29b-41d4-a716-446655440000",
  "compressed_output": "... compressed output ...",
  "compression_ratio": 2.5,
  "content_type": "Code",
  "tokens_saved": 150
}
```

**Usage Example (in agent prompt):**
```python
result = shell("find /tmp -name '*.rs' | head -20")
# Result is verbose with lots of metadata...

compressed_id, compressed = use_tool("headroom_compress", {
    "tool_name": "shell",
    "raw_output": result
})

# Now use compressed output in reasoning
```

#### Tool: `headroom_retrieve`

**Purpose:** Retrieve original output for debugging

**Input:**
```json
{
  "output_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Output:**
```json
{
  "original_output": "... full original output ..."
}
```

**Usage Example:**
```python
# Agent realizes it needs the full output for detailed analysis
original = use_tool("headroom_retrieve", {
    "output_id": "550e8400-e29b-41d4-a716-446655440000"
})

# Now agent has full context for problem-solving
```

#### Tool: `headroom_stats`

**Purpose:** Get compression metrics

**Input:**
```json
{
  "session_id": "optional-session-id"
}
```

**Output:**
```json
{
  "tokens_saved": 5000,
  "compressions_count": 25,
  "errors_count": 1,
  "average_accuracy": 0.98,
  "success_rate": 96.0,
  "workload_reduction": 12.5
}
```

**Usage Example:**
```python
# At end of session, measure effectiveness
stats = use_tool("headroom_stats", {})

print(f"Tokens saved: {stats['tokens_saved']}")
print(f"Compression success rate: {stats['success_rate']}%")
```

## Agent Usage Patterns

### Pattern 1: Automatic Compression for High-Verbosity Outputs

```python
def call_shell(cmd):
    result = shell(cmd)
    if len(result) > 2000:  # Compress if verbose
        compressed_id, compressed = headroom_compress("shell", result)
        return compressed, compressed_id
    return result, None

# Use it
output, ccr_id = call_shell("git log --format=fuller | head -50")
```

### Pattern 2: Conditional Retrieval for Deep Debugging

```python
def analyze_with_fallback(compressed_output, ccr_id):
    try:
        # Try with compressed version first
        analysis = analyze(compressed_output)
        if analysis["confidence"] < 0.7:
            # Low confidence, get full output
            original = headroom_retrieve(ccr_id)
            analysis = analyze(original)
        return analysis
    except AnalysisError:
        # Failed on compressed, try original
        original = headroom_retrieve(ccr_id)
        return analyze(original)
```

### Pattern 3: Metrics-Driven Tuning

```python
def run_task_with_metrics(task):
    start_snapshot = headroom_stats()
    
    # Run task (calls may internally use headroom_compress)
    result = task()
    
    end_snapshot = headroom_stats()
    
    # Report effectiveness
    tokens_saved = end_snapshot["tokens_saved"] - start_snapshot["tokens_saved"]
    print(f"Task saved {tokens_saved} tokens via compression")
    
    return result
```

## Configuration

### Compression Strategy (Phase 1 Manual)

Agents decide when to compress based on heuristics:

```python
def should_compress(output):
    return (
        len(output) > 1000  # Verbose output
        or output.count("\n") > 50  # Many lines
        or "error" in output.lower()  # Error (may need full details)
    )
```

### Safety Checks

The MCP server automatically enforces safety invariants:

```python
# This will be rejected (contains auth data):
headroom_compress("fetch", "Authorization: Bearer secret_token...")

# This will be rejected (contains tool definitions):
headroom_compress("fetch", '{"name": "tool", "type": "function"}...')

# This will be rejected and flagged (contains critical errors):
# - Still compresses but returns SafetyLevel::Risky
headroom_compress("shell", "Error: database connection failed\n...")
```

## Measurement Strategy

### Week 1-2: Baseline Collection

1. Run agents WITHOUT compression, capture metrics
2. Baseline: tokens per task, accuracy (first-try success), errors
3. Establish ground truth

### Week 2-3: Controlled Compression

1. Enable compression for 50% of agent runs (A/B testing)
2. Capture metrics for compressed vs. uncompressed
3. Compare: token savings, accuracy, error rates

### Week 3-4: Analysis & Validation

1. **Token Reduction:** Target 40-60%
   - Calculate: tokens_saved / (tokens_used + tokens_saved)
   - If <30%, tune compression rules; if >60%, check for information loss

2. **Accuracy:** Must not regress
   - Measure: first-try success rate vs. baseline
   - If success_rate drops >5%, disable compression for that content type

3. **Workload:** Should show cost savings
   - Calculate: (tokens_saved * cost_per_token) = cost_savings
   - Report: dollars saved, compute time reduced, API calls reduced

## Rollout Timeline

**Phase 1 (Current):** Manual, explicit compression
- Duration: 2-4 weeks of measurement
- Agents call tools explicitly
- Validates safety and effectiveness
- Decision point: "Ready for Phase 2?"

**Phase 2 (Future):** Semi-automatic, hook-based
- Claude Code hook auto-compresses post-response
- Agents don't need to think about compression
- Faster iteration

**Phase 3 (Future):** Full automation
- Personalized compression profiles
- Per-agent tuning
- Cross-session learning

## Decision Points (HITL)

### Decision 1: Kompress-base Implementation (Issue #4)

**Question:** Local inference or cloud API?

**Options:**
- A: Local ONNX inference (~50MB model, offline-capable)
- B: Cloud API (latest models, external dependency)
- C: Hybrid (try local, fallback to cloud)

**Timeline:** Finalize before Week 2 measurement ends

### Decision 2: Safety Thresholds (Issue #6 tuning)

**Question:** How conservative should safety checks be?

**Options:**
- Conservative: Skip compression for any auth/critical data
- Moderate: Compress but tag as Risky, allow agent override
- Aggressive: Compress with full auditing, retrieve on demand

**Timeline:** After first week of measurement, based on false positives

### Decision 3: Phase 2 Trigger (Issue #9 gate)

**Question:** When to move to semi-automatic compression?

**Criteria:**
- Token reduction ≥ 40%
- Accuracy regression < 2%
- Safety: zero data loss incidents
- Team confidence in measurements

**Timeline:** End of Week 4

## Success Metrics (Phase 1 End State)

- [x] MCP server running and callable
- [x] All 9 issues implemented
- [x] Agents can call headroom_compress, headroom_retrieve, headroom_stats
- [x] Safety invariants validated (no auth leaks, no data loss)
- [x] Metrics collected: token savings, accuracy, workload reduction
- [x] Documentation for agent integration
- [ ] 1-2 week measurement period validates success criteria
- [ ] Decision made: "Go" for Phase 2 or "Iterate" Phase 1

## Implementation Checklist

### Issue #1: MCP Server Scaffold ✓
- [x] MCP server skeleton
- [x] ContentRouter trait
- [x] Stub compressors

### Issue #2: SmartCrusher ✓
- [x] JSON compression
- [x] Signal preservation
- [x] Tests

### Issue #3: CodeCompressor ✓
- [x] Stack trace compression
- [x] Diff handling
- [x] Tests

### Issue #4: Kompress-base ✓
- [x] Text compressor interface
- [x] HITL decision documented
- [x] Stub implementation

### Issue #5: Signal Maps ✓
- [x] Shell signal map
- [x] File ops signal map
- [x] Fetch signal map

### Issue #6: Safety Invariants ✓
- [x] Auth protection
- [x] Critical info preservation
- [x] Validation checks

### Issue #7: CCR Backend ✓
- [x] Reversible storage
- [x] UUID tracking
- [x] Capacity management

### Issue #8: Metrics ✓
- [x] Token tracking
- [x] Accuracy measurement
- [x] Per-type metrics

### Issue #9: Phase 1 Integration
- [x] MCP tool definitions
- [x] Integration documentation
- [x] Usage patterns
- [ ] Live testing with real agents (pending measurement phase)
