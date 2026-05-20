Status: ready-for-agent

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Add adaptive expansion order: use higher p in dense regions where force gradients are large, lower p in sparse regions. This is a design decision (HITL) because it requires:
- A per-node error estimator (e.g., based on cell multipole moment magnitude or neighbor distance)
- A decision rule mapping estimator → p
- Variable-order M2L/L2L/L2P that accept per-node p values

The CLI gains `--adaptive-p` flag that enables this mode. The `--convergence` diagnostic reports effective order distribution.

This is the most complex optimization and should be tackled last, after the baseline M2L is fully optimized.

## Acceptance criteria

- [ ] Per-node error estimator is implemented (examples: multipole magnitude ratio, neighbor cell size ratio)
- [ ] Decision rule maps estimator to p ∈ {2,4,6} per node
- [ ] M2L/L2L/L2P accept per-node p values
- [ ] `--adaptive-p` flag enables adaptive mode
- [ ] `--convergence` reports effective order distribution
- [ ] Accuracy at same mean p is comparable to fixed-p mode
- [ ] All existing tests still pass

## Blocked by

- 10-m2l-hierarchical.md (needs optimized M2L baseline)
