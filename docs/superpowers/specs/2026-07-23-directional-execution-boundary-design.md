# Directional Execution Boundary Design

## Goal

Make every report use one canonical, direction-aware execution boundary so that
entry, confirmation, stop, targets, prerequisites, and actionability cannot
contradict one another across the trader plan, decision panel, IC discipline,
action guides, probability view, or rendered report.

The immediate regression is task `4d756c57-1ad4-4464-966a-54ff5b36b010` for
`09868.HK`: a valid research-stage short plan was overwritten by incompatible
later fields, then a post-build repair stretched the target to an arbitrary
1.5 reward/risk ratio.

## Selected Approach

Use a structured `ExecutionBoundary` as the only source of execution values.
It is built once after agent outputs are available and before downstream report
views are derived. It uses typed fields and numeric checks only; it never
infers business logic from localized or LLM prose.

Prompt-only fixes are insufficient because later agent stages can still
contradict earlier numeric plans. A final-rendering-only repair is also
insufficient because it can make the visible prices disagree with the
calculation, action guides, and target conditions.

## Boundary Model

The model contains:

- `direction`: bullish, bearish, or neutral.
- `confirmation_price` and `confirmation_mode`: including daily-close and
  volume requirements.
- `entry_price`, `stop_price`, `stage_one_target`, and `final_target`.
- `minimum_reward_risk`, `actual_reward_risk`, and `active_execution_allowed`.
- typed prerequisite codes for volume confirmation, borrow quantity, borrow
  fee, borrow term, borrow market depth, and missing cash-flow treatment.
- a structured substitute-evidence summary for cash balance, net debt,
  short-debt coverage, and operating-loss trend. It explicitly states that
  these indicators do not replace operating cash flow, capital expenditure,
  or free cash flow.
- a short and an extended horizon for staged targets.

For a viable bearish setup, the invariant is:

`final_target <= stage_one_target < entry < confirmation < stop`

For a viable bullish setup, the inverse ordering is enforced. Entry is placed
past confirmation in the trade direction. Its buffer is volatility-aware and
clamped between 0.25% and 0.5% of confirmation, using 0.1 ATR before clamping.
For near-price confirmations, daily close plus at least 1.5 times 20-day
average volume is mandatory.

## Source And Normalization

The LLM schema will emit a typed primary execution path. The normalizer gives
that path explicit precedence over prose fields, validates its direction and
numeric order, and derives only missing fields from price and ATR evidence.
If a path is invalid or incomplete, the boundary is non-actionable and the
report remains research-only; it does not fabricate a trade.

All consumer views read the canonical boundary. Legacy report fields remain
for compatibility but are synchronized from it, rather than acting as peer
sources that can overwrite it.

## Risk And Actionability Rules

An active trade requires a valid boundary, a reward/risk ratio of at least
2.0, and every prerequisite required by its direction. For shorts, the borrow
quantity, fee, term, depth, and account authorization prerequisites are all
mandatory. Their absence allows Underweight, avoidance, or reduction of an
existing long, but prohibits an active short recommendation.

If the technical target does not reach 2.0 reward/risk, the system preserves
the real target and marks active execution blocked. It must never move a
target solely to meet a minimum ratio. Stage-one and final targets are shown
with their associated horizons so a distant target is not presented as a
near-term expectation.

Cash-flow absence is represented as a typed evidence limitation. The report
will show its structured substitute evidence and the limitation that it does
not prove cash conversion or runway; it cannot silently disappear into a
generic diagnostic warning.

## Rendering And Localization

The backend returns keys, enums, numbers, and typed prerequisite codes. The
frontend maps these to Chinese and other supported interface languages. No
backend business branch uses `contains`, localized strings, or LLM narrative
text.

The report should show one explicit execution status: research-only, waiting
for confirmation, blocked by prerequisites, or executable. This status is
shared by the decision panel, trade setup quality, action guides, IC
discipline, and trader plan.

## Regression Coverage

Tests under `tests/` will cover:

- bearish and bullish price-order invariants;
- entry/confirmation separation with volatility-aware buffers;
- symmetric confirmation-distance calculation for bearish setups;
- preservation of a sub-2.0 target and blocking of active execution;
- staged targets and horizons;
- borrow prerequisites blocking a short without blocking Underweight;
- cash-flow substitute evidence and limitation rendering data;
- synchronization of all report consumers from the same boundary;
- the `09868.HK` regression fixture with conflicting legacy fields.

Verification includes focused Rust tests, the complete backend suite, report
regeneration for `09868.HK`, and inspection of persisted structured values and
the rendered report.
