# HITL Decisions: Phase 1 Implementation

**Decision Date:** 2026-06-21  
**Status:** Complete  
**Next Review:** After Week 2 measurement results

---

## Decision 1: Kompress-base Inference Strategy

### Question
How should KompressBase (text compressor) perform inference?

**Options Evaluated:**

| Option | Local ONNX | Cloud API | Hybrid |
|--------|-----------|-----------|--------|
| Latency | ~50-200ms | 200-500ms | ~50-200ms |
| Offline capable | ✓ | ✗ | Partial |
| Model updates | Manual | Automatic | Automatic |
| Dependencies | ONNX Runtime | HTTP client | Both |
| Deployment | Self-contained | Requires service | Both |
| Cost | Infrastructure | Per-call API | Blended |

### Decision: **HYBRID (Option C)**

### Rationale

**Why not Option A (Local Only)?**
- ONNX Runtime adds ~50MB dependency
- Model downloads during first run
- Slower startup on cold start
- Model updates require code releases
- Not suitable for resource-constrained environments

**Why not Option B (Cloud Only)?**
- High latency (200-500ms per compression)
- Requires external service dependency
- Network reliability risk
- Per-call costs can accumulate
- Not suitable for offline/air-gapped deployments

**Why Hybrid (Option C)?**
- ✓ Best of both worlds: fast local fallback, latest models via cloud
- ✓ Resilience: works offline with local model, scales with cloud
- ✓ Cost optimization: use local for common cases, cloud for edge cases
- ✓ Flexibility: migrate between strategies without code changes

### Implementation Plan

**Phase 1 (Current):** Local ONNX as primary
- Download model on first run (configurable path)
- Fast inference with ~50-200ms latency
- 100% offline capable
- Fallback: If ONNX fails, use heuristic compression

**Phase 2 (Post-measurement):** Add cloud fallback
- Configure cloud API endpoint and key
- If local model unavailable or too old, use cloud
- Cache results to avoid repeated cloud calls
- Graceful degradation

**Configuration Example:**
```toml
[compression.kompress_base]
# Local ONNX inference
local_model_path = "~/.cache/headroom/kompress-base-v1.onnx"
local_enabled = true
local_timeout_ms = 500

# Cloud API fallback
cloud_enabled = true
cloud_endpoint = "https://api.kompress.dev/compress"
cloud_api_key = "${KOMPRESS_API_KEY}"
cloud_timeout_ms = 2000

# Strategy
strategy = "hybrid"  # local_first, cloud_first, or hybrid
fallback = "heuristic"  # fallback if both fail
```

### Next Steps
1. Post-measurement decision: based on Week 1-2 results
2. If token savings < 40%: prioritize cloud integration
3. If token savings sufficient: keep local-only for now
4. Month 2: Implement cloud fallback if Phase 2 approved

---

## Decision 2: Safety Thresholds

### Question
How conservative should safety checks be when detecting risky content?

**Options Evaluated:**

| Aspect | Conservative | Moderate | Aggressive |
|--------|--------------|----------|-----------|
| Auth detection | Block all | Block, tag Risky | Log only |
| Error preservation | Always preserve | Preserve if critical | Heuristic |
| Tool definitions | Never compress | Tag Risky | Compress |
| Function signatures | Never compress | Preserve, compress metadata | Compress all |
| Retrieval on demand | Required | Optional | Discouraged |

### Decision: **MODERATE (Option B)**

### Rationale

**Why not Conservative (Option A)?**
- Too restrictive: many false positives
- Reduces compression effectiveness (defeats goal of 40-60%)
- All errors trigger Unsafe: legitimate stderr output blocked
- Agents can't override even when safe
- Prevents measurement of true compression benefit

**Why not Aggressive (Option C)?**
- Risks data loss on first-time runs
- Auth headers might slip through
- No audit trail for safety violations
- Violates "safety invariant" requirement
- Team liability if secrets leak

**Why Moderate (Option B)?**
- ✓ Preserves safety while allowing compression
- ✓ Provides SafetyLevel feedback (Safe/Risky/Unsafe)
- ✓ Agents see what's being done
- ✓ Risky items preserved and retrievable
- ✓ Clear audit trail for compliance
- ✓ Balances security and effectiveness

### Implementation Details

**SafetyLevel Mapping:**

```rust
pub enum SafetyLevel {
    Safe,      // Compress freely (no auth, no errors)
    Risky,     // Compress but preserve all critical info + preserve for retrieval
    Unsafe,    // Reject compression (auth data or tool definitions)
}
```

**Decision Tree:**

```
Input: content

1. Has auth data? (Bearer, API key, etc.)
   YES → Unsafe (reject compression)
   
2. Has tool definitions? ({"name": ..., "type": ...})
   YES → Unsafe (reject compression)

3. Has critical errors? (Error:, Exception:, Fatal:)
   YES → Risky (compress but preserve all error lines)

4. Has function signatures? (fn, pub fn, def)
   YES → Risky (compress but preserve signatures)

5. Has "Retrieve" request patterns?
   YES → Risky (agent may need full output)

6. Everything else
   → Safe (compress aggressively)
```

**Storage & Retrieval:**

```
Moderate Strategy:
  - ALL compressions stored in CCR (100% coverage)
  - Risky items flagged in metadata
  - Agents can retrieve Risky items anytime
  - CCR cleanup policy: keep for 24 hours minimum
  
Query example:
  SELECT * FROM ccr_records 
  WHERE safety_level = 'Risky'
  AND created_at > NOW() - INTERVAL '24 hours'
```

### Agent Visibility

Agents always see:
```json
{
  "output_id": "...",
  "compressed_output": "...",
  "safety_level": "Risky",  // ← Explicit feedback
  "tokens_saved": 150,
  "note": "Original preserved for retrieval (contains error messages)"
}
```

Agents can decide:
- Use compressed version (most of the time)
- Retrieve original if needed (for detailed error analysis)

### Next Steps
1. Week 1-2: Measure false positive rate on Risky level
2. Week 3 analysis: If too many false positives → adjust patterns
3. If too conservative → shift toward Moderate-Aggressive
4. Final tuning after measurement phase

---

## Decision 3: Phase 2 Trigger Criteria

### Question
When is the system ready to move from manual (Phase 1) to automatic (Phase 2) compression?

### Decision: **GO-GATE Model (Structured Decision)**

### Criteria (All Must Be Met)

```
PRIMARY GATES (Technical):
  [✓] Token reduction ≥ 40%
      └─ Measured: (tokens_before - tokens_after) / tokens_before
      └─ Target range: 40-60%
      └─ Failure: < 40% indicates ineffective compression
      
  [✓] Accuracy no regression (< 2%)
      └─ Measured: success_rate_experimental - success_rate_baseline
      └─ Threshold: ≥ -0.02 (allow up to 2% drop)
      └─ Failure: > 2% drop indicates information loss
      
  [✓] Safety: Zero data loss
      └─ Measured: CCR retrieval byte-equality check
      └─ Threshold: 100% of retrievals match originals
      └─ Failure: Any corruption triggers immediate halt

SECONDARY GATES (Operational):
  [✓] Workload reduction: Positive cost savings
      └─ Measured: (baseline_tokens - experimental_tokens) × cost_per_token
      └─ Threshold: > $0 cost savings
      └─ Failure: Negative savings = worse performance
      
  [✓] Safety review: Zero auth leaks
      └─ Measured: Manual audit of compression logs
      └─ Threshold: No auth patterns in compressed outputs
      └─ Failure: Any leaked secrets = ABORT immediately
      
  [✓] Team confidence: Consensus decision
      └─ Participants: Tech lead, PM, Security, Product
      └─ Threshold: ≥ 3/4 vote GO
      └─ Failure: < 3/4 → ITERATE or ABANDON

FAILURE BRANCHES (If Criteria Not Met):

If Token Reduction < 40%:
  ├─ Analyze per-content-type metrics
  ├─ Identify worst-performing types
  ├─ Adjust signal maps or compression aggressiveness
  ├─ Re-run focused measurement (1 week)
  └─ Re-evaluate gate

If Accuracy Drop > 2%:
  ├─ Identify which task types affected
  ├─ Check if Safety::Risky is suppressing important info
  ├─ Compare error preservation before/after
  ├─ Disable compression for affected types
  ├─ Re-run measurement (1 week)
  └─ Re-evaluate gate

If Any Data Loss:
  ├─ IMMEDIATE: Disable compression service
  ├─ URGENT: Analyze root cause
  ├─ Post-mortem: What failed? How to prevent?
  ├─ Fix root cause
  ├─ Extensive testing before re-enabling
  └─ Restart measurement from Week 1
```

### Decision Matrix

```
╔════════════════════════════════════════════════════════╗
║                 PHASE 2 GO/NO-GO MATRIX              ║
╠════════════════════════════════════════════════════════╣
║ Token ≥40% │ Accuracy <2% │ No Data Loss │ DECISION   ║
╠════════════════════════════════════════════════════════╣
║    YES     │     YES      │     YES      │ GO → Phase2║
║    YES     │     YES      │      NO      │ ABORT      ║
║    YES     │      NO      │     YES      │ ITERATE    ║
║     NO     │     YES      │     YES      │ ITERATE    ║
║    YES     │      NO      │      NO      │ ABORT      ║
║    ...any combination with NO to safety...│ ABANDON   ║
╚════════════════════════════════════════════════════════╝
```

### Phase 2 Rollout (If GO)

**Timeline (2 weeks):**
```
Day 1:    Implement automatic hook in Claude Code
Day 2-3:  QA and internal testing (10% of agents)
Day 4-5:  Canary rollout (25% of agents)
Day 6-7:  Wider rollout (50% of agents)
Day 8-10: Full rollout (100% of agents)
Day 11-14: Monitor stability and metrics
```

**Automatic Compression Strategy:**
```python
# Claude Code hook (pseudo-code)
def after_tool_response(tool_name: str, output: str):
    # Auto-compress if:
    # 1. Output is large (>1000 bytes)
    # 2. Content type is known (JSON/Code/Text)
    # 3. Safety level is Safe or Risky
    
    if len(output) > 1000 and should_auto_compress(output):
        try:
            compressed_id, compressed = headroom_compress(tool_name, output)
            # Replace output with compressed version
            # Agent uses compressed output automatically
            return compressed
        except Exception as e:
            # Fallback: return original
            logging.error(f"Auto-compression failed: {e}")
            return output
    
    return output  # Don't compress if too small or unsafe
```

### Next Steps

**Immediate (Week 1):**
- Deploy measurement infrastructure
- Start baseline collection
- Set up dashboards and alerts

**Week 2:**
- Complete A/B testing
- Collect metrics
- Begin analysis

**Week 3:**
- Finish analysis
- Prepare gate review materials
- Pre-meeting: technical review with team

**Week 4:**
- Hold go/no-go meeting
- Vote on decision
- If GO: Begin Phase 2 implementation
- If ITERATE: Plan tuning sprint
- If ABANDON: Post-mortem and lessons learned

---

## Summary Table

| Decision | Option | Rationale | Status |
|----------|--------|-----------|--------|
| Kompress-base | Hybrid | Resilience + performance trade-off | ✅ Decided |
| Safety Thresholds | Moderate | Balance security and effectiveness | ✅ Decided |
| Phase 2 Trigger | Go-gate model | Structured, measurable criteria | ✅ Decided |

## Sign-Off

These decisions enable Phase 1 measurement and Phase 2 planning. Revisit after Week 2 measurement results if direction changes.

**Approved:** Implementation Team  
**Date:** 2026-06-21  
**Next Review:** End of Week 2 (measurement update)
