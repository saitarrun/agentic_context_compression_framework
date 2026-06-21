# Measurement Plan: Phase 1 Validation (Weeks 1-4)

**Goal:** Validate that compression achieves 40-60% token reduction with no accuracy regression, enabling go/no-go decision for Phase 2.

---

## Week 1-2: Baseline & Controlled Rollout

### Week 1: Baseline Collection (No Compression)

**Setup:**
- Run agents WITHOUT any compression tools available
- Capture baseline metrics for 50+ diverse tasks
- Establish ground truth for comparison

**Metrics to Capture:**
```
Per task:
  - total_tokens_used
  - first_try_success (boolean)
  - retry_count (number of iterations)
  - errors_encountered (count)
  - task_completion_time (seconds)
  - tool_outputs (store raw outputs)
  
Aggregate:
  - avg_tokens_per_task
  - success_rate (%)
  - avg_retries_per_failed_task
  - error_rate (%)
```

**Sample Tasks:**
- Code review (bash grep, reading files)
- Git operations (git log, diff)
- API testing (curl/fetch requests)
- Data analysis (large JSON responses)
- Error debugging (stack trace analysis)

**Baseline Example:**
```
Task: Review 100-line Python file
  Tokens used: 4,500
  First-try success: YES
  Retries: 0
  Errors: 0
  Time: 45 seconds
  
Aggregate baseline (50 tasks):
  Avg tokens: 3,200
  Success rate: 72%
  Avg retries: 1.2
  Error rate: 5%
```

### Week 2: A/B Test with Compression (50/50 Split)

**Setup:**
- Enable compression tools for 50% of agents
- Disable compression for remaining 50% (control group)
- Run identical task suite on both groups
- Randomize which agent gets which variant

**Compression Strategy:**
Agents that have compression enabled use this heuristic:

```rust
fn should_compress(output: &str) -> bool {
    output.len() > 1000  // Verbose output
        || output.lines().count() > 50  // Many lines
        || output.contains("Error") || output.contains("Failed")  // Errors
}
```

**Metrics to Capture (both groups):**
```
Control group (no compression):
  - Same baseline metrics as Week 1

Experimental group (with compression):
  - All baseline metrics PLUS:
  - tokens_before_compression
  - tokens_after_compression
  - compression_ratio_per_output
  - output_ids_for_retrieval
  - compressions_attempted (count)
  - compressions_failed (count)
  - retrievals_used (count)
  - safety_level_detected (Safe/Risky/Unsafe)
```

**Comparison Matrix:**

| Metric | Control | Experimental | Delta |
|--------|---------|--------------|-------|
| Avg tokens/task | 3,200 | ? | ? |
| Success rate | 72% | ? | ? |
| Avg retries | 1.2 | ? | ? |
| Error rate | 5% | ? | ? |

**Decision Logic:**
```
IF experimental_tokens < control_tokens * 0.6:
  ✓ Token reduction sufficient (40% target met)
ELSE:
  ⚠ Need tuning or different strategy

IF experimental_success_rate >= control_success_rate - 0.02:
  ✓ No accuracy regression
ELSE:
  ⚠ Compression harming accuracy, investigate
  
IF safety_unsafe_count == 0 AND retrieval_errors == 0:
  ✓ Safety invariants holding
ELSE:
  ⚠ Data loss or safety violation detected
```

---

## Week 2-3: Analysis Phase

### Analysis 1: Token Reduction

**Calculate:**
```
Token savings = Σ(tokens_before - tokens_after) for all compressions
Token savings % = (token_savings / total_tokens_used) * 100

Success criteria: ≥ 40% (target: 40-60%)
```

**If insufficient savings (<40%):**
1. Check which content types compress poorly
2. Analyze per-type metrics from Issue #8
3. Tune signal maps (Issue #5)
4. Consider more aggressive compression rules
5. Re-run subset of tasks

**If excessive savings (>70%):**
1. Check if information is being lost
2. Compare success rates carefully
3. Verify CCR retrievals are working
4. May indicate over-aggressive noise removal

### Analysis 2: Accuracy (First-Try Success Rate)

**Calculate:**
```
Accuracy delta = experimental_success_rate - baseline_success_rate
Success criteria: delta ≥ -0.02 (no more than 2% regression)
```

**If accuracy drops >2%:**
1. Identify which task types are affected
2. Analyze compression logs for those tasks
3. Check if safety level Risky is being triggered
4. Hypothesis testing: Is accuracy drop due to:
   - Lost error information?
   - Lost context?
   - False compression (safe content marked as noise)?
5. Adjust signal maps or disable compression for problematic content types

**If accuracy improves:**
1. Analyze why (less token consumption → better reasoning?)
2. Document the pattern
3. Use as evidence for Phase 2 go-ahead

### Analysis 3: Workload Reduction

**Calculate:**
```
Total API costs:
  - Baseline: (baseline_tokens * cost_per_token)
  - Experimental: (experimental_tokens * cost_per_token)
  
Cost savings = Baseline_cost - Experimental_cost
Percentage savings = (Cost_savings / Baseline_cost) * 100

Performance delta:
  - Baseline: avg_task_completion_time
  - Experimental: avg_task_completion_time
  - Speedup = (Baseline_time - Experimental_time) / Baseline_time * 100
```

**Expected results:**
- Cost savings correlate with token savings
- Performance may improve (fewer tokens = faster API calls)
- Memory usage may improve (smaller context buffers)

### Analysis 4: Safety Validation

**Checks:**
```
✓ Safety::Unsafe count == 0 (no auth leaks)
✓ Safety::Risky count tracked (errors preserved?)
✓ CCR retrieval errors == 0 (no data loss)
✓ Byte-faithful verification (sample retrievals match originals)
✓ No data corruption in storage
```

**If violations found:**
1. Identify specific patterns that caused issues
2. Update safety rules
3. Consider disabling compression for those patterns
4. Re-test after fixes

---

## Week 3-4: Gate Decision

### Decision Matrix

```
GATE CRITERIA:

✓ Token reduction ≥ 40% AND
✓ Accuracy regression < 2% AND
✓ Zero data loss incidents AND
✓ Safety invariants holding AND
✓ Team confidence validated

OUTCOME OPTIONS:

[A] GO → Phase 2
    - Implement automatic compression (Claude Code hooks)
    - Roll out to all agents over 2 weeks
    - Continue monitoring metrics

[B] ITERATE → Phase 1 Tuning
    - Analyze root causes of shortfall
    - Adjust compression parameters
    - Re-run measurement (2-3 weeks)
    - Re-evaluate gate criteria

[C] ABANDON → Shelve for next quarter
    - Document findings
    - Create postmortem
    - Plan improvements for future attempt
```

### Go/No-Go Meeting (End of Week 4)

**Required participants:**
- Tech lead (implementation owner)
- Project manager (metrics analysis)
- Security lead (safety review)
- Product owner (business impact)

**Agenda:**
1. Present Week 1-2 baseline data
2. Present Week 2-3 A/B test results
3. Review Analysis 1-4 findings
4. Assess gate criteria (met/not met)
5. Vote: GO / ITERATE / ABANDON
6. If GO: Plan Phase 2 rollout (2-week timeline)
7. If ITERATE: Create tuning plan
8. If ABANDON: Schedule postmortem

---

## Measurement Instrumentation

### Data Collection Points

```
Compression Tool Call:
  - timestamp
  - tool_name (shell/file/fetch)
  - input_size (bytes)
  - output_size (bytes)
  - compression_ratio
  - tokens_saved
  - content_type (detected)
  - safety_level
  - compressor_used
  - success (true/false)
  - error_message (if failed)

Retrieval Call:
  - timestamp
  - output_id
  - success (true/false)
  - original_size (bytes)
  - retrieved_size (bytes)
  - byte_equality_check (true/false)

Agent Task:
  - task_id
  - task_name
  - start_time
  - end_time
  - tokens_used_total
  - compressions_attempted
  - compressions_successful
  - retrievals_used
  - first_try_success
  - retry_count
  - errors_encountered
  - final_result (success/failure)
```

### Logging & Export

**CSV Export Format:**
```
timestamp,event_type,metric_name,value,task_id,variant
2026-06-22T10:30:45Z,compression,tokens_saved,150,task_001,experimental
2026-06-22T10:31:12Z,retrieval,byte_equality,true,task_001,experimental
2026-06-22T10:32:00Z,task_complete,success_rate,1,task_001,experimental
```

**Dashboard Queries:**
```sql
-- Weekly summary
SELECT 
  variant,
  AVG(tokens_used) as avg_tokens,
  COUNT(CASE WHEN first_try_success THEN 1 END) / COUNT(*) as success_rate,
  AVG(retry_count) as avg_retries
FROM task_metrics
WHERE week = 1
GROUP BY variant

-- Compression effectiveness
SELECT
  content_type,
  COUNT(*) as attempts,
  SUM(tokens_saved) as total_saved,
  AVG(compression_ratio) as avg_ratio
FROM compression_events
WHERE success = true
GROUP BY content_type
```

---

## Risk Mitigation

### Risk 1: Data Corruption During Measurement

**Mitigation:**
- Enable byte-equality checks on all retrievals
- Spot-check CCR storage integrity hourly
- Maintain backup of all original outputs
- Abort measurement if any corruption detected

### Risk 2: Safety Violations (Auth Leaks)

**Mitigation:**
- Pre-filter tasks to avoid sensitive data
- Monitor for auth-related strings in compressed outputs
- Daily security audit of compression logs
- Isolate measurement to non-production agent instances

### Risk 3: Measurement Confounds

**Mitigation:**
- Use randomized task assignment (control ↔ experimental)
- Fix task set (same tasks for both groups)
- Run simultaneously (same time-of-day patterns)
- Control for external variables (API rate limits, network latency)

### Risk 4: Insufficient Data

**Mitigation:**
- Target 50+ tasks minimum
- Require at least 100 compression operations
- Extended measurement window if needed
- Accept statistical uncertainty, report confidence intervals

---

## Success Criteria Summary

| Criterion | Target | Method |
|-----------|--------|--------|
| Token Reduction | ≥ 40% | Measure tokens_before / tokens_after |
| Accuracy | < 2% regression | Compare success_rate baseline vs. experimental |
| Safety | Zero data loss | Check retrieval byte-equality, audit logs |
| Workload | Positive savings | Calculate cost × tokens for both variants |
| Confidence | Team consensus | Go/no-go vote by leadership |

---

## Timeline

```
Week 1       Week 2       Week 3       Week 4
│────────│────────│────────│────────│
Baseline  A/B Test  Analysis  Decision
(No comp)  (50/50)   & Review  Meeting
                     Tuning?   GO/ITERATE/
                               ABANDON
```

**Deliverables by Week:**
- **Week 1:** Baseline metrics report
- **Week 2:** A/B test results (preliminary)
- **Week 3:** Full analysis + recommendation
- **Week 4:** Go/no-go decision + Phase 2 plan (if GO)
