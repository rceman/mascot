# Perf A/B — first-show regression check

Interleaved runs, same machine (i9-9900K / RTX 4070, hardware D3D11), strict
alternation A1 → B1 → A2 → B2, `perf --runs 5` each:

- **A** = `4ab080d` (pre-gallery), exported via `git archive` to scratch and
  built there (`ab-4ab080d\target\release\mascot-ui-lab.exe`)
- **B** = `c39f45c + working-tree cleanup` — behaviourally identical to
  `03eae71` (the difference is lab-internal colour plumbing, receipts and
  an unused test binding; no runtime path changed)

```powershell
# run strictly in this order
A: mascot-ui-lab.exe perf --out ab\A1.json --runs 5
B: mascot-ui-lab.exe perf --out ab\B1.json --runs 5
A: mascot-ui-lab.exe perf --out ab\A2.json --runs 5
B: mascot-ui-lab.exe perf --out ab\B2.json --runs 5
```

## first_show_ms (per run)

| Set | Values | Median |
|---|---|---|
| A1 | 775.0, 564.9, 628.4, 622.0, 590.1 | 622.0 |
| B1 | 608.5, 559.4, 601.7, 624.2, 576.2 | 601.7 |
| A2 | 623.5, 639.4, 632.0, 635.6, 604.7 | 632.0 |
| B2 | 650.6, 609.0, 615.8, 601.9, 603.8 | 609.0 |
| **all A** | | **626.0** |
| **all B** | | **606.2** |

Medians differ by ~20 ms (B marginally faster) — the earlier 819 ms median
was cold-start variance, not a code effect.

## Warm open p50 / submit→response p50 (per run)

| Set | open p50 | submit p50 |
|---|---|---|
| A1 | 3.72, 3.85, 3.66, 3.70, 3.75 | 6.57, 6.09, 6.15, 6.30, 5.97 |
| B1 | 3.82, 3.58, 3.75, 4.11, 3.69 | 6.55, 6.41, 6.59, 6.01, 5.81 |
| A2 | 3.82, 4.18, 3.78, 3.78, 3.88 | 6.60, 7.21, 5.98, 6.07, 6.14 |
| B2 | 3.76, 3.72, 3.91, 3.74, 3.86 | 6.38, 6.27, 5.87, 5.80, 6.06 |
| **median** | A 3.78 / B 3.75 | A 6.15 / B 6.16 |

## Stage breakdown (last run of each set)

| Stage | A ms / MB | B ms / MB |
|---|---|---|
| renderer | 267 / 54.4 | 270 / 54.2 |
| rig | 354 / 82.8 | 362 / 83.2 |
| fonts+painter | 402 / 82.8 | 410 / 83.2 |
| richedit | 469 / 83.0 | 477 / 83.4 |
| sprite | 557 / 143.5 | 569 / 143.8 |
| first_show | 624 / 148.7 | 652 / 149.5 |
| after_gpu_trim | 697 / 85.7 | 710 / 86.0 |

Identical shape; deltas are within run-to-run noise.

Raw JSONs: `A1.json`, `B1.json`, `A2.json`, `B2.json` in this directory.
