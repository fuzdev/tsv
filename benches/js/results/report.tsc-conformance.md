# tsc_conformance run — committed report

Oracle: tsgo committed `.errors.txt` baselines (bind + merge + flow family). Deterministic — wall-clock excluded.

## Denominators

- in-scope tests: 9389
- in-scope variants: 9888
- expect-clean graded / clean pass: 4475 / 4475
- baselined + parsed: 4504
- family graded / family-positive: 4134 / 143

## Family (dup 2300 / 2451 / 2567 / 2528 + merge 2397 / 2649 / 2664 / 2671; flow 7027 / 7028)

- match: 581 (dup 547, flow 34)
- missing: 37 (dup 11, flow 26)
  - by cause: merge-path 0, lib-conflict 0, late-bound 11, cfa 26, other 0
- extra (GATE=0): 0
- span mismatch: 0

## Per-code table

| code | match | missing |
| --- | --- | --- |
| TS2300 | 423 | 11 |
| TS2397 | 4 | 0 |
| TS2451 | 56 | 0 |
| TS2528 | 35 | 0 |
| TS2567 | 26 | 0 |
| TS2664 | 3 | 0 |
| TS7027 | 31 | 26 |
| TS7028 | 3 | 0 |

## Related-info channel (matched primaries)

- match / missing / extra / span-mismatch: 51 / 0 / 0 / 0

## Carve-outs

- recovery-AST rule (a): 370 (family-positive 11)
- moduleDetection variants (inert for family): 1

## Parse-divergence census

- parse-rejected: 909 (no baseline 5, TS1xxx-only 443, other 461)
- script-goal retries: 59
- crash-excluded (tracked): 0

## Lib base

- lib files bound / sets folded: 107 / 50
