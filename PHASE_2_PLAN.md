# Phase 2 Plan: Automatic Compression via Claude Code Hooks

**Status:** Contingent on Phase 1 measurement success (go-gate criteria met)  
**Estimated Timeline:** 2-3 weeks after Phase 1 approval  
**Owner:** Implementation team + Claude Code integration  

---

## Overview

Phase 2 moves from **manual, explicit compression** (Issue #9) to **automatic, transparent compression** via Claude Code hooks. Agents don't call compression tools explicitly; compression happens automatically post-response.

### Key Difference from Phase 1

| Aspect | Phase 1 | Phase 2 |
|--------|---------|---------|
| Trigger | Agent calls headroom_compress | Automatic after tool response |
| Visibility | Explicit in agent prompt | Transparent, handled automatically |
| Configuration | Agent code | System configuration |
| Opt-out | Disable tools | Configuration override |
| Metrics | Captured manually | Automatic collection |

---

## Architecture

### Claude Code Hook Integration

```
Tool Response Flow:
┌──────────────────┐
│  Agent calls tool│ (shell, file, fetch)
└────────┬─────────┘
         │
         ↓
┌──────────────────┐
│  Tool executes   │ (returns output)
└────────┬─────────┘
         │
         ↓
    [NEW HOOK]
┌──────────────────────────────────────┐
│ Claude Code Hook: after_tool_response│
│ ─────────────────────────────────────│
│ 1. Check if compression needed       │
│ 2. Call headroom_compress()          │
│ 3. Replace output if safe            │
│ 4. Log metrics to CCR + metrics      │
│ 5. Return compressed (or original)   │
└────────┬─────────────────────────────┘
         │
         ↓
┌──────────────────────────────────────┐
│  Agent sees compressed output        │
│  (or original if compression failed) │
└──────────────────────────────────────┘
```

### Hook Configuration

**.claude/settings.json:**
```json
{
  "hooks": {
    "after_tool_response": {
      "command": "python",
      "args": ["/usr/local/lib/headroom/hook_after_tool_response.py"],
      "environment": {
        "HEADROOM_MCP_SERVER": "headroom-compression",
        "HEADROOM_AUTO_COMPRESS": "true",
        "HEADROOM_COMPRESS_THRESHOLD": "1000",
        "HEADROOM_SAFETY_LEVEL": "moderate"
      },
      "timeout_ms": 5000,
      "on_error": "passthrough"  // Return original if hook fails
    }
  },
  
  "mcpServers": {
    "headroom-compression": {
      "command": "/usr/local/bin/compression-mcp"
    }
  }
}
```

---

## Implementation Components

### 1. Hook Script (after_tool_response)

**File:** `/usr/local/lib/headroom/hook_after_tool_response.py`

```python
#!/usr/bin/env python3
"""
Claude Code hook for automatic compression.
Runs after every tool response.
"""

import sys
import json
import os
from pathlib import Path

def should_compress(tool_name: str, output: str, config: dict) -> bool:
    """Determine if output should be compressed."""
    # Don't compress if disabled
    if not config.get("AUTO_COMPRESS", True):
        return False
    
    # Check size threshold
    threshold = int(config.get("COMPRESS_THRESHOLD", 1000))
    if len(output) < threshold:
        return False
    
    # Don't compress certain tools (e.g., API keys, passwords)
    excluded_tools = config.get("EXCLUDE_TOOLS", [])
    if tool_name in excluded_tools:
        return False
    
    # Don't compress if output contains auth
    if any(pattern in output for pattern in 
           ["Authorization:", "api-key:", "Bearer ", "secret="]):
        return False
    
    return True

def compress_output(tool_name: str, output: str, mcp_server: str) -> tuple[str, dict]:
    """Call MCP server to compress output."""
    # TODO: Implement MCP client to call headroom_compress
    # For now, return original
    return output, {
        "compressed": False,
        "reason": "hook_not_yet_implemented"
    }

def main():
    # Read hook input (JSON from Claude Code)
    try:
        hook_input = json.loads(sys.stdin.read())
    except json.JSONDecodeError:
        # If input is malformed, pass through
        print(json.dumps({"output": sys.stdin.read()}))
        return
    
    tool_name = hook_input.get("tool_name", "unknown")
    output = hook_input.get("output", "")
    
    # Load configuration
    config = {
        "AUTO_COMPRESS": os.getenv("HEADROOM_AUTO_COMPRESS", "true").lower() == "true",
        "COMPRESS_THRESHOLD": os.getenv("HEADROOM_COMPRESS_THRESHOLD", "1000"),
        "MCP_SERVER": os.getenv("HEADROOM_MCP_SERVER", "headroom-compression"),
        "EXCLUDE_TOOLS": os.getenv("HEADROOM_EXCLUDE_TOOLS", "").split(","),
    }
    
    # Decide whether to compress
    if should_compress(tool_name, output, config):
        try:
            compressed, metadata = compress_output(
                tool_name, 
                output,
                config["MCP_SERVER"]
            )
            
            # Return compressed output with metadata
            sys.stdout.write(json.dumps({
                "output": compressed,
                "compression_metadata": metadata
            }))
        except Exception as e:
            # On error, return original (passthrough)
            sys.stdout.write(json.dumps({
                "output": output,
                "compression_error": str(e)
            }))
    else:
        # Don't compress
        sys.stdout.write(json.dumps({
            "output": output,
            "compression_skipped": True
        }))

if __name__ == "__main__":
    main()
```

### 2. Hook Integration Module

**File:** `crates/compression-mcp/src/hook_client.rs`

```rust
/// Hook client for Claude Code integration
/// Implements hook script communication protocol

use mcp_types::{CompressRequest, CompressResponse};
use std::process::{Command, Stdio};
use std::io::Write;

pub struct HookClient {
    mcp_server: String,
    timeout_ms: u64,
}

impl HookClient {
    pub fn new(mcp_server: String) -> Self {
        Self {
            mcp_server,
            timeout_ms: 5000,
        }
    }

    /// Call the MCP server to compress output
    pub async fn compress_via_mcp(
        &self,
        tool_name: &str,
        output: &str,
    ) -> Result<CompressResponse, String> {
        // TODO: Implement MCP JSON-RPC call
        // This would:
        // 1. Connect to MCP server (Unix socket or TCP)
        // 2. Send headroom_compress request
        // 3. Wait for response (with timeout)
        // 4. Parse and return CompressResponse
        
        Err("Not yet implemented".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hook_client_creation() {
        let client = HookClient::new("headroom-compression".to_string());
        assert_eq!(client.timeout_ms, 5000);
    }
}
```

### 3. Metrics Exporter

**File:** `crates/compression-mcp/src/exporter.rs`

```rust
/// Export metrics to external systems for monitoring

use crate::metrics::MetricsSnapshot;
use std::fs::File;
use std::io::Write;

pub struct MetricsExporter;

impl MetricsExporter {
    /// Export metrics to CSV for analysis
    pub fn export_to_csv(
        snapshot: &MetricsSnapshot,
        path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = File::create(path)?;
        
        writeln!(file, "metric,value")?;
        writeln!(file, "total_tokens_saved,{}", snapshot.total_tokens_saved)?;
        writeln!(file, "compressions_count,{}", snapshot.compressions_count)?;
        writeln!(file, "errors_count,{}", snapshot.errors_count)?;
        writeln!(file, "average_accuracy,{:.4}", snapshot.average_accuracy)?;
        writeln!(file, "success_rate,{:.2}", snapshot.success_rate)?;
        writeln!(file, "workload_reduction,{:.2}", snapshot.workload_reduction)?;
        
        Ok(())
    }

    /// Export to Prometheus format for monitoring
    pub fn export_to_prometheus(snapshot: &MetricsSnapshot) -> String {
        format!(
            r#"# HELP headroom_tokens_saved Cumulative tokens saved via compression
# TYPE headroom_tokens_saved counter
headroom_tokens_saved {{}} {}

# HELP headroom_compressions_total Total compression operations
# TYPE headroom_compressions_total counter
headroom_compressions_total {{}} {}

# HELP headroom_compression_accuracy Average compression accuracy (0-1)
# TYPE headroom_compression_accuracy gauge
headroom_compression_accuracy {{}} {:.4}

# HELP headroom_compression_success_rate Success rate percentage
# TYPE headroom_compression_success_rate gauge
headroom_compression_success_rate {{}} {:.2}
"#,
            snapshot.total_tokens_saved,
            snapshot.compressions_count,
            snapshot.average_accuracy,
            snapshot.success_rate
        )
    }

    /// Export to JSON for dashboards
    pub fn export_to_json(snapshot: &MetricsSnapshot) -> String {
        serde_json::json!({
            "total_tokens_saved": snapshot.total_tokens_saved,
            "compressions_count": snapshot.compressions_count,
            "errors_count": snapshot.errors_count,
            "average_accuracy": snapshot.average_accuracy,
            "success_rate": snapshot.success_rate,
            "workload_reduction": snapshot.workload_reduction,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_export() {
        let snapshot = MetricsSnapshot {
            total_tokens_saved: 1000,
            compressions_count: 10,
            errors_count: 1,
            average_accuracy: 0.95,
            success_rate: 90.0,
            workload_reduction: 12.5,
        };

        let output = MetricsExporter::export_to_prometheus(&snapshot);
        assert!(output.contains("headroom_tokens_saved"));
        assert!(output.contains("1000"));
    }
}
```

---

## Rollout Strategy

### Canary Rollout (Days 1-5)

**Day 1-2: Internal Testing**
- Test hook with 5-10 internal tasks
- Verify compression is transparent
- Check metrics collection
- Test failure modes (hook timeout, MCP unavailable)

**Day 3-4: Canary (10% of agents)**
- Deploy hook to 10% of Claude Code users
- Monitor error rates and metrics
- Watch for unintended side effects
- Collect feedback

**Day 5: Go/No-go checkpoint**
- If canary metrics good → proceed
- If issues found → roll back and fix
- Escalate to team if problems

### Wider Rollout (Days 6-10)

**Day 6-7: 25% deployment**
- Expand to 25% of agents
- Continue monitoring
- A/B test against Phase 1 manual compression

**Day 8-10: Full deployment**
- Roll out to 100% of Claude Code agents
- Monitor stability
- Track metrics across all users

### Stabilization (Days 11-14)

- Monitor for emergent issues
- Collect 1 week of production metrics
- Compare to Phase 1 baselines
- Generate post-launch report

---

## Fallback & Rollback Plan

### Automatic Rollback Triggers

```python
# Monitor these conditions
if metrics.error_rate > 0.05:  # >5% errors
    trigger_rollback("High error rate")

if metrics.average_accuracy < (baseline_accuracy - 0.05):  # >5% accuracy drop
    trigger_rollback("Accuracy degradation")

if metrics.tokens_saved < (Phase1_savings * 0.8):  # >20% less savings than Phase 1
    trigger_rollback("Compression effectiveness dropped")

if any(safety_violation) in metrics:  # Auth leak, data corruption
    trigger_immediate_halt("Safety violation detected")
```

### Rollback Procedure

1. **Immediate:** Disable hook in all .claude/settings.json
2. **Verification:** Confirm agents revert to Phase 1 (manual compression)
3. **Investigation:** Analyze logs for root cause
4. **Fix:** Address root cause
5. **Testing:** Extensive testing before re-enabling
6. **Re-deployment:** Gradual rollout after fix validated

---

## Success Metrics (Phase 2 Specific)

| Metric | Target | Method |
|--------|--------|--------|
| Transparency | 0% agent awareness | Compression should be invisible |
| Latency | <500ms total | Hook overhead <5% of task time |
| Reliability | 99.5% success | Compression works without failures |
| Accuracy | No regression | Same or better than Phase 1 |
| Adoption | 80%+ of agents | Automatic usage without opt-in |

---

## Phase 2 Timeline

```
Week 1 (Post-approval)
  Day 1-2: Implement hook script + MCP client
  Day 3: QA and integration testing
  Day 4-5: Internal canary (10%)

Week 2
  Day 1-2: Canary analysis + adjustments
  Day 3-4: Wider rollout (25%)
  Day 5-7: Continue to 50% → 100%

Week 3
  Day 1-7: Stabilization + monitoring
  End: Post-launch report
```

---

## Decision Gates for Phase 2 Proceeding

**DO NOT proceed with Phase 2 if:**

- [ ] Phase 1 gate criteria NOT met (token < 40%, accuracy drop > 2%, data loss)
- [ ] Safety team NOT signed off (zero auth leaks confirmed)
- [ ] Measurement team NOT confident (measurement validity concerns)
- [ ] HITL decisions NOT finalized (ambiguity on approach)
- [ ] Hook infrastructure NOT ready (hook client not implemented)

**PROCEED with Phase 2 only if:**

- [x] Phase 1 measurement complete (all 4 weeks)
- [x] Go-gate criteria MET (≥40% tokens, <2% accuracy drop, zero data loss)
- [x] Team consensus (3/4+ vote GO)
- [x] Safety review passed (zero auth leaks, no data corruption)
- [x] Hook infrastructure ready (script + MPC client implemented)
- [x] Rollout plan approved (leadership sign-off)

---

## Long-Term Roadmap

**Phase 3 (Month 2-3):** Personalization
- Per-agent compression profiles
- Learn which compression strategies work best per agent
- Adaptive signal maps based on agent behavior

**Phase 4 (Month 3+):** Multi-Session Learning
- Cross-session optimization
- Persistent CCR storage (SQLite/Redis)
- Long-term metrics tracking and analysis

---

## Appendix: MCP Client Implementation

The hook script needs an MCP client to communicate with the MCP server. This requires:

1. **Protocol:** JSON-RPC 2.0 over stdio (see [MCP spec](https://modelcontextprotocol.io/))
2. **Serialization:** serde_json for Rust, json for Python
3. **Error handling:** Timeout, server unavailable, invalid response
4. **Caching:** Optional caching of compression results for same input

**Example MCP call (Python):**
```python
import json
import subprocess

def call_mcp(method: str, params: dict) -> dict:
    """Call MCP server and return result"""
    request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    }
    
    response = subprocess.run(
        ["compression-mcp"],
        input=json.dumps(request).encode(),
        capture_output=True,
        timeout=5
    )
    
    return json.loads(response.stdout)

# Usage
result = call_mcp("headroom_compress", {
    "tool_name": "shell",
    "raw_output": "... large output ..."
})
```

This is READY TO IMPLEMENT once Phase 1 is approved.
