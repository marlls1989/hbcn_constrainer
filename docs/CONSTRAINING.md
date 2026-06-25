# The `constrain` command

`hbcn constrain` turns an HBCN model and a target cycle time into Genus-compatible SDC timing
constraints. This document describes every flag, the two constraint-generation algorithms, and the
linear programs they solve.

```
hbcn constrain <input> --sdc <out.sdc> -t <cycle_time> -m <min_delay> [options]
```

By default the input is read as an [HBCN](INPUT_FORMATS.md) (`.hbcn`); pass `--structural` to read a
structural graph (`.graph`) instead, in which case it is expanded to an HBCN first.

## Flags

| Flag | Type / default | Effect |
|---|---|---|
| `<input>` | path (positional) | The circuit to constrain. HBCN by default; structural graph with `--structural`. |
| `--structural` | off | Read `<input>` as a `.graph` and expand it to an HBCN (see `--no-forward-completion`). |
| `--sdc <PATH>` | path (required) | Write the SDC constraints here. Always emitted. |
| `-t, --cycle-time <T>` | f64 (required) | Target cycle time `T`. The LP closes timing so every token cycle's delays sum to `tokens · T`. |
| `-m, --minimal-delay <m>` | f64 (required) | Floor `m` for every path's max delay, and the lower bound of the LP delay variables. |
| `--csv <PATH>` | optional | Tabular per-place dump: `src,src_dir,dst,dst_dir,cost,max_delay,min_delay`. |
| `--rpt <PATH>` | optional | Human-readable report with the critical cycles. |
| `--vcd <PATH>` | optional | Waveform of transition arrival times. |
| `--no-proportional` | off | Use the **pseudoclock** algorithm instead of the default **proportional** one. |
| `--no-forward-completion` | off | Structural expansion only: do **not** raise a forward path's weight to the register's completion-detection cost when that cost exceeds the virtual delay. |
| `-f, --forward-margin <pct>` | u8 `0..100`, optional | Add a min/max window on forward (propagation) paths: `min = (1 − pct/100) · max`. Emits `set_min_delay`. |
| `-b, --backward-margin <pct>` | u8 `0..100`, optional | Add a min/max window on backward (acknowledge) paths (see [Margins](#margins-f--b)). |

The top-level `hbcn -v/--verbose` flag makes `constrain` print progress (including which algorithm
ran) to stderr. Note that `-f`/`-b` take a **percentage** (e.g. `-f 20`), internally converted to the
ratio `1 − pct/100` (so `-f 20` ⇒ `0.80`, `-f 0` ⇒ `1.0`).

## The cycle-time model (shared by both algorithms)

An HBCN is a marked graph: **transitions** are nodes, **places** are directed edges. Each place
`p : u → v` carries a weight `w(p)` (its virtual delay / logical-depth cost), a `token(p)` flag (part
of the initial marking), and an `internal(p)` flag (places inside an expanded register). Every channel
has four places — data propagation (`+→+`), spacer propagation (`−→−`), and two acknowledges
(`+→−`, `−→+`); each is an independent edge with its own weight (see [INPUT_FORMATS.md](INPUT_FORMATS.md)).

Both algorithms introduce an arrival-time variable `a(t) ≥ 0` per transition and a delay variable
`d(p)` per place, tied together by the **separation constraint**

```
d(p) + a(u) − a(v) = token(p) · T          for every place p : u → v
```

Summing this around any directed cycle cancels the arrival times and leaves

```
Σ d(p)  =  (number of tokens in the cycle) · T
```

i.e. the delays around every token cycle sum to exactly `tokens · T`. This is what pins the design to
the requested cycle time `T`; the two algorithms differ only in how they choose the individual `d(p)`
within that envelope, and in what they maximise.

Both solves are run through the LP backend (`coin_cbc` or `gurobi`); the values read back (arrival
times, delays, slacks, objective) are rounded to 8 significant digits to mask solver floating-point
noise.

## Proportional (default)

Distributes the cycle-time budget across paths **in proportion to their virtual delays**, so a deep
path is allowed a larger delay than a shallow one — there is no single clock period.

Per place `p` it creates `max(p) ≥ m`, `min(p) ≥ 0`, `slack(p) ≥ 0`, and a single global `factor ≥ 0`:

```
maximise   factor

subject to, for every place p : u → v
  (1)  max(p) + a(u) − a(v) = token(p) · T          separation / cycle-time
  (2)  max(p) = w(p) · factor + slack(p)            proportional distribution
       max(p) ≥ m,  min(p) ≥ 0,  slack(p) ≥ 0,  a(·) ≥ 0
  (3)  margin constraints on min(p), external places only  (see Margins)
```

Constraint (2) forces `max(p) ≥ w(p) · factor`, and maximising `factor` pushes every path's allowance
up until the tightest (critical) cycle saturates: there `slack(p) = 0`, so `max(p) = w(p) · factor`
and, by the cycle identity, `factor = tokens · T / Σ w(p)` for that cycle. Every place's max delay is
then `w(p) · factor` (plus slack off the critical cycle) — proportional to its weight. Paths left at
the floor `m` carry no real constraint and are dropped from the SDC.

The reported clock period (the SDC `create_clock` period) is the minimal delay `m`.

> Because each place has its own `max`/`min`/`slack`, the data and spacer propagation places (and the
> two acknowledges) are constrained independently. Earlier the two places of a node pair shared one
> triple; with differing weights that made constraint (2) force `factor = 0`. See
> [`examples/hbcn/distinct.hbcn`](../examples/hbcn/distinct.hbcn).

## Pseudoclock (`--no-proportional`)

Constrains **every external path to at least a common period**, like a synchronous clock, and
maximises that period. Internal (intra-register) paths only get the `m` floor.

Per place `p` it creates one delay `d(p) ≥ m`, plus a single global `pseudo_clock ≥ 0`:

```
maximise   pseudo_clock

subject to, for every place p : u → v
  (1)  d(p) + a(u) − a(v) = token(p) · T            separation / cycle-time
  (2)  d(p) ≥ pseudo_clock     if p is external
       d(p) ≥ m                if p is internal
       a(·) ≥ 0
```

Maximising `pseudo_clock` raises the uniform floor on external paths as high as the cycle-time
envelope (1) permits. The solved `pseudo_clock` becomes the SDC `create_clock` period, and each
place's `max(p) = d(p)`; a `max` equal to the clock adds nothing and is dropped. Pseudoclock produces
only max constraints (no min), so `-f`/`-b` have no effect here.

**Proportional vs pseudoclock:** proportional gives each path a budget scaled to its logical depth
(tighter on shallow logic, looser on deep logic); pseudoclock gives all register-to-register paths one
shared period. Proportional is the default and is generally preferred for cyclic circuits.

## Margins (`-f` / `-b`)

By default only a max delay is generated per path. The margins add a **minimum** delay, producing a
`set_min_delay`/`set_max_delay` window (useful for hold-style and handshake relationships). They apply
only to the proportional algorithm and only to external (non-internal) places. Let `f = 1 − f_pct/100`
and `b = 1 − b_pct/100`, and for a backward (acknowledge) place `p` let `q` be its matching forward
place (data-ack pairs with forward-data, spacer-ack with forward-spacer):

| Place kind | `-f` only | `-b` only | both `-f` and `-b` |
|---|---|---|---|
| Forward (propagation) | `min(p) = f · max(p)` | — | `min(p) = f · max(p)` |
| Backward (acknowledge) | `min(p) = max(q) − min(q)` and `min(p) ≤ max(p)` | `min(p) = b · max(p)` | `min(p) = max(q) − min(q)` and `min(p) ≤ b · max(p)` |

With no margin given, `min(p)` stays at 0 and no `set_min_delay` is emitted.

## Forward completion (`--no-forward-completion`)

This affects only the **structural-graph expansion** (`--structural`), not the LP. When a structural
edge is expanded, its two forward (propagation) places take a `forward_cost` and its two acknowledge
places a `backward_cost` derived from the register fan-in. By default (`forward_completion` on)
`forward_cost = max(virtual_delay, completion_cost + base)`, so a path whose register completion
detection is slower than its logic is floored at that completion cost. `--no-forward-completion` sets
`forward_cost = virtual_delay` instead. HBCN input is unaffected — it carries its own per-place
weights.

## Outputs

- **SDC** (`--sdc`, required): a `create_clock` line followed by one `set_max_delay`/`set_min_delay`
  per place. Each `-through` clause is qualified by its endpoint's transition direction — a `Data`
  (`+`) transition rises, a `Spacer` (`−`) falls — so propagation paths are `-rise_through … -rise_through`
  / `-fall_through … -fall_through` (positive unate) and acknowledges are `-rise_through … -fall_through`
  / `-fall_through … -rise_through` (negative unate). A max delay within 0.1 % of the clock period, or a
  negligible (`≤ 0.001`) min delay, is omitted.
- **CSV** (`--csv`): one row per place with both endpoint directions and the cost / max / min.
- **Report** (`--rpt`): the critical cycles and their slacks.
- **VCD** (`--vcd`): transition arrival times as a waveform.

## Worked example

```
hbcn constrain examples/hbcn/distinct.hbcn --sdc out.sdc -t 1000 -m 10
```

The single channel `a ↔ reg1` has four distinct place weights (100 / 40 / 30 / 30). Proportional sets
`factor = 1000 / (100+40+30+30) = 5`, so the SDC carries four independent constraints —
`set_max_delay 500` (forward-data, rise→rise), `200` (forward-spacer, fall→fall), and `150` for each
acknowledge (rise→fall and fall→rise) — and `create_clock -period 10`.
