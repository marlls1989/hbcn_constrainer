# Input formats

`hbcn` reads two text formats:

- **Structural graph** (`.graph`) — a description of the circuit's connectivity.
- **HBCN** (`.hbcn`) — the expanded Half-Buffer Channel Network, a marked graph of
  signed transitions and the timed places between them.

The `expand` subcommand converts the first into the second. `analyse` and
`constrain` read an HBCN by default, or a structural graph when `--structural`
is given; `expand` always reads a structural graph.

```
            --structural                       default
 .graph ───────────────► analyse / constrain ◄──────── .hbcn
   │                                                      ▲
   └──────────────── hbcn expand ────────────────────────┘
```

Both grammars are token-based and **whitespace-insensitive**: newlines and
indentation carry no meaning, so the one-entry-per-line layout used throughout
this document is a readability convention, not a requirement. The grammars are
defined with LALRPOP in
[`src/structural_graph/parser.lalrpop`](src/structural_graph/parser.lalrpop) and
[`src/hbcn/parser/parser.lalrpop`](src/hbcn/parser/parser.lalrpop); this document
describes them in prose. Worked inputs of each kind live under
[`examples/structural_graphs/`](examples/structural_graphs/) and
[`examples/hbcn/`](examples/hbcn/).

Numeric literals have no exponent and may be integer or decimal. In the structural
graph they are non-negative; in the HBCN format a delay may carry a leading `-` (see
*Delays* below). A literal large enough to overflow to floating-point infinity is
rejected rather than silently propagated into the timing model.

---

## Structural graph (`.graph`)

A node-per-line adjacency list. Each entry declares one circuit node and the
channels that node drives:

```
<Type> "<name>" [("<target>", <delay>), ("<target>", <delay>), ...]
```

### Grammar

```
graph     ::= node*
node      ::= type string "[" adjacency "]"
type      ::= "Port" | "DataReg" | "NullReg" | "ControlReg" | "UnsafeReg"
adjacency ::= ( tuple ("," tuple)* ","? )?        # comma-separated; empty list allowed
tuple     ::= "(" string "," number ")"
string    ::= '"' /[^"]*/ '"'                     # any character except a double quote
number    ::= /[0-9]+(\.[0-9]+)?/                 # non-negative integer or decimal
```

### Fields

- **`<Type>`** — the component kind (see the table below).
- **`"<name>"`** — the node name, in double quotes. Any character except `"` is
  permitted and there is no escape sequence. Names must be unique across the file
  (a repeated name is a "multiple definitions" error). Every `<target>` should
  name a node declared elsewhere in the same file.
- **adjacency list** — zero or more `("<target>", <delay>)` pairs inside
  `[ ... ]`, separated by commas (a trailing comma is permitted). `<target>` is
  the name of another node that this one drives. An empty list `[]` marks a sink,
  such as a primary output port.
- **`<delay>`** — the channel's *virtual delay*: a number modelling the
  combinational propagation delay along the channel from this node to `<target>`.
  It becomes the weight of the corresponding forward places in the HBCN.

### Component types

| Type | Role | Expansion in the HBCN |
|------|------|-----------------------|
| `Port` | External interface (primary input or output) | A single node with no internal logic and a modelled cost of 0. |
| `NullReg` | Plain pipeline register | A single node. |
| `ControlReg` | Control register | A single node, with a higher modelled cost than `NullReg`. |
| `DataReg` | Data register with completion detection | Three nodes — `name`, `name/s0`, `name/s1` — joined by internal channels. |
| `UnsafeReg` | Register without full completion detection | Two nodes — `name` and `name/s0`. |

The "cost" above is part of the timing model the conversion applies (see
[`src/hbcn/structural_graph.rs`](src/hbcn/structural_graph.rs)); it is not written
in the input.

### The `port:` name prefix

The prefix `port:` is **reserved for ports**: only `Port` components may use a name
that begins with `port:`, and any other component that does is rejected. A
`port:`-prefixed name is how a node is recognised as a port when an HBCN is read
back (see below), and the `constrain` subcommand maps such a name to an instance
pin in the generated SDC. Ports without the prefix are accepted but are not
identifiable as ports once serialised to the HBCN text format.

### Example

A small cyclic circuit, from
[`examples/structural_graphs/cyclic.graph`](examples/structural_graphs/cyclic.graph):

```
Port "a" [("b", 20)]
DataReg "b" [("b", 15), ("c", 10)]
Port "c" []
```

- input port `a` drives register `b` with a virtual delay of 20;
- register `b` feeds back to itself with delay 15 and forwards to output port `c`
  with delay 10;
- output port `c` is a sink (empty adjacency list).

---

## HBCN (`.hbcn`)

The expanded, timing-level representation produced by `hbcn expand`. It is a marked
graph: the nodes are **transitions** (signed events at circuit nodes) and each line
of the file is a **place** (a timed, optionally token-bearing edge) joining two
transitions.

```
[*] <source-transition> => <target-transition> : <delay>
```

### Grammar

```
hbcn       ::= edge*
edge       ::= "*"? transition "=>" transition ":" delay
transition ::= ("+" | "-") node
node       ::= "{" /(\\[{}]|[^}])*/ "}"           # literal { and } escaped as \{ \}
delay      ::= number | "(" number "," number ")"
number     ::= /-?[0-9]+(\.[0-9]+)?/              # integer or decimal; may be negative
```

### Transitions

A transition is an event at a circuit node, written as a sign followed by a
brace-quoted node name:

- **`+{name}`** — a *data* transition (the data phase of the four-phase handshake).
- **`-{name}`** — a *spacer* transition (the return-to-zero / null phase).

The name is enclosed in braces. A literal brace within the name is escaped
TCL-style — `{` as `\{` and `}` as `\}` — so a node named `a{0}` is written
`+{a\{0\}}`. A name beginning with `port:` denotes a port (e.g. `+{port:input}`);
any other name denotes a register, including the `/s0` and `/s1` internal nodes
that `DataReg` and `UnsafeReg` expand into. This prefix is how ports and registers
are told apart when an HBCN is read back.

### Places

Each line is a place between two transitions. Its direction classifies it:

- a place whose endpoints share a sign (data→data or spacer→spacer) is a
  **forward** place, modelling data flow along a channel;
- a place whose endpoints differ in sign (data→spacer or spacer→data) is a
  **backward** place, modelling the acknowledgement (handshake) path.

Each channel of the structural graph expands into four places: a forward-data
place, a forward-spacer place, and two backward places.

### Token marking

A leading **`*`** marks a place that initially holds a token — part of the marked
graph's starting marking, which encodes the initial state of each channel's
handshake. On unmarked lines the serialiser writes two spaces in place of the `*`
purely for alignment; because the format is whitespace-insensitive, leading
whitespace has no effect on input.

### Delays

A delay is either a single maximum or a minimum–maximum pair:

- **`<max>`** — a maximum only, with no minimum. This is the form `expand` emits.
- **`(<min>,<max>)`** — both bounds, for example `(1.0,2.5)`. This form is written
  when serialising a *solved* or *constrained* HBCN, and is also accepted on input.

Each bound is a number, integer or decimal, and **may be negative** — negative delays
are physically real (e.g. slew/recovery, where an output begins to switch before its
input has fully transitioned). Whitespace around the comma is permitted.

`analyse` reads a place's `max` as a *real delay*, so a negative value lowers the
computed cycle time. `constrain` instead reads it as a logical-depth *weight* used to
distribute the cycle-time budget into fair per-path constraints; there a small or
negative value simply makes the path non-critical and is assigned the smallest legal
constraint (`min_delay`). The structural `.graph` virtual delays remain non-negative.
See [`examples/hbcn/neg.hbcn`](examples/hbcn/neg.hbcn) for a minimal channel whose
cycle time is lowered by a negative delay.

### Example

The expansion of a single self-looping `DataReg`, from
[`examples/hbcn/loop.hbcn`](examples/hbcn/loop.hbcn). The register `a` expands into
three nodes (`a`, `a/s0`, `a/s1`); its three channels contribute four places each.
Initially-marked places carry a leading `*`, and every delay is max-only (as always
for `expand` output):

```
  +{a} => +{a/s0} : 10
* -{a} => -{a/s0} : 10
  +{a/s0} => -{a} : 20
  -{a/s0} => +{a} : 20
* +{a/s0} => +{a/s1} : 10
  -{a/s0} => -{a/s1} : 10
  +{a/s1} => -{a/s0} : 20
  -{a/s1} => +{a/s0} : 20
  +{a/s1} => +{a} : 50
  -{a/s1} => -{a} : 50
  +{a} => -{a/s1} : 20
* -{a} => +{a/s1} : 20
```

Reading the first channel (`a` → `a/s0`): the forward-data place
`+{a} => +{a/s0}` and forward-spacer place `-{a} => -{a/s0}` carry the channel's
virtual delay (10), while the two backward places `+{a/s0} => -{a}` and
`-{a/s0} => +{a}` carry the computed handshake delay (20). The forward-spacer place
holds the initial token.
