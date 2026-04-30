# rlox

A bytecode virtual machine interpreter for the [Lox](https://craftinginterpreters.com/) programming language, implemented in Rust. This is a Rust port of the `clox` VM from Robert Nystrom's *Crafting Interpreters*, with several design decisions and extensions that deviate from the reference implementation.

---

## Architecture Overview

```
Source Code
    │
    ▼
Scanner (src/compiler/scanner.rs)
    │  tokenizes raw source into a flat stream of Tokens
    ▼
Parser / Compiler (src/compiler/mod.rs, parser.rs)
    │  single-pass Pratt parser — no AST; emits bytecode directly
    ▼
Chunk (src/chunk.rs)
    │  bytecode instruction stream + constants pool + line info
    ▼
VM (src/vm.rs)
    │  stack-based interpreter — walks the bytecode stream
    ▼
Output / Side Effects
```

The compiler is a **single-pass, recursive-descent Pratt parser** — it produces bytecode directly without building an intermediate AST. The VM is a simple **stack machine**.

---

## Notable Design Decisions & Extensions

### 1. `OpCode::Constant24` — Supports more than 255 Constants

The standard `OP_CONSTANT` instruction encodes its operand in a single byte, limiting a chunk to **256 unique constants**. `rlox` adds `OpCode::Constant24`, whose operand is a **24-bit little-endian integer**, raising the ceiling to **16,777,216 constants**.

```
OP_CONSTANT     <1 byte index>          — for constants 0–255
OP_CONSTANT24   <byte0> <byte1> <byte2> — for constants 256–16,777,215
```

`Chunk::write_constant` selects the right opcode automatically:

```rust
if idx < 256 {
    self.code.push(OpCode::Constant as u8);
    self.code.push(idx as u8);
    // 2 line entries
} else {
    self.code.push(OpCode::Constant24 as u8);
    let (b0, b1, b2) = Self::resolve_index(idx);
    // 3-byte operand + 4 line entries
}
```

The VM's `read_constant` mirrors this, accepting an `is_long` flag to read either 1 or 3 bytes.

---

### 2. String Interning via `string-interner`

Rather than heap-allocating a `String` per string value, all strings are **interned** into a global `StringInterner` (backed by `string_interner = "0.19.0"`). Each unique string is stored exactly once, and `Value::String` carries a compact `SymbolU32` *(4 byte)* handle instead of the string data itself.

```rust
pub enum Value {
    String(SymbolU32),  // 4-byte symbol, not an owned String
    Number(f64),
    Boolean(bool),
    Nil,
    // ...
}
```

The interner is a lazily-initialized, `Mutex`-protected global in `src/data_structures/interner.rs`:

```rust
pub(crate) static STRING_INTERNER: Interner =
    LazyLock::new(|| Mutex::new(StringInterner::default()));

pub fn intern(string: &str) -> SymbolU32 { ... }
pub fn get_string(symbol: SymbolU32) -> Option<String> { ... }
```

**Consequences:**
- **String equality is an integer comparison** — `SymbolU32` equality is `O(1)` with no character scanning.
- **String concatenation** interns the result, so the concatenated string is also deduplicated.
- There is no separate heap-allocated object list for strings; the interner owns all string memory.

---

### 3. Deduplicating the Constants Pool (`add_if_absent`)

The compiler calls `Chunk::add_if_absent` when storing identifiers (variable names) as constants. Rather than pushing a new `Value::String` entry every time the same variable name appears in source, this method scans the pool and returns the existing index if a matching value is already present:

```rust
pub fn add_if_absent(&mut self, value: Value) -> usize {
    for (index, constant) in self.constants.iter().enumerate() {
        if constant == &value {
            return index;
        }
    }
    self.add_constant(value)
}
```

This reduces redundant entries in the constants pool for programs with many references to the same variable names.

---

### 4. No Separate `OP_NOT_EQUAL`, `OP_LESS_EQUAL`, `OP_GREATER_EQUAL`

Rather than adding dedicated opcodes for `!=`, `<=`, and `>=`, the compiler composes existing opcodes:

| Source | Emitted bytecode |
|--------|-----------------|
| `a != b` | `OP_EQUAL`, `OP_NOT` |
| `a <= b` | `OP_GREATER`, `OP_NOT` |
| `a >= b` | `OP_LESS`, `OP_NOT` |

This keeps the instruction set minimal. The VM has total freedom over its instruction set — it only needs to produce correct behavior, not mirror source syntax.

---

### 5. Custom Hash Table (`HashTable`) with Open Addressing

`src/data_structures/mod.rs` implements a hand-rolled `HashTable<SymbolU32, Value>` using **open addressing with linear probing** and **FNV-1 hashing** over the `SymbolU32` key. This is used for the global variable store in the VM.

- `insert` replaces the value for an existing key.
- `delete` tombstones and **rehashes the trailing cluster** to preserve probe invariants — avoiding the ghost-entry bug that a naïve delete causes with open addressing.
- `get_key_index` implements the core probe loop.

---

### 6. Pratt Parser with a Static `RULES` Table

Operator precedence and associativity are encoded in a `static RULES: [ParseRule; 40]` array indexed by `Kind as u8`. Each entry holds an optional prefix parse function, an optional infix parse function, and a `Precedence` level — the classic Pratt approach. Because the table is `const`-initialized with function pointers (not closures), it requires no heap allocation.

```rust
static RULES: [ParseRule; 40] = {
    let mut rules = [ParseRule::default(); 40];
    rules[Kind::Plus as usize] =
        ParseRule::new_infix(|compiler, _| compiler.binary(), Precedence::Term);
    // ...
    rules
};
```

---

## Supported Language Features

- Arithmetic: `+`, `-`, `*`, `/`, unary `-`
- Comparison: `==`, `!=`, `<`, `<=`, `>`, `>=`
- Logical: `!` (not), `nil` falsey semantics
- Types: `number` (f64), `string` (interned), `bool`, `nil`
- String concatenation with `+`
- `print` statement
- Global variable declaration (`var`) and assignment
- Expression statements (result discarded via `OP_POP`)
- Single-line comments (`//`)

---

## Project Structure
```
src/
├── main.rs                    — entry point / REPL stub
├── lib.rs                     — crate root, module declarations
├── runtime/
│   ├── vm.rs                 — VM, interpreter loop, InterpretResult
│   ├── gc.rs                 — Mark-sweep garbage collector, Trace trait
│   ├── heap.rs               — Heap data_structure, GcValue (all reference and complex types)
│   └── lox_errors.rs          — VmError type
├── core/
│   ├── mod.rs                 — module declarations
│   ├── chunk.rs               — Chunk, bytecode helpers
│   ├── opcode.rs              — OpCode definitions
│   ├── value.rs               — Value enum, arithmetic operator impls
│   └── lox_errors.rs          — VmError type
├── compile/
│   ├── mod.rs                 — module declarations
│   ├── compiler.rs            — Compiler, Pratt parser, ParseRule table
│   ├── parser.rs              — Parser, error reporting
│   ├── scanner.rs             — Scanner / lexer
│   └── token.rs               — Token, Kind enum
└── data_structures/
    ├── mod.rs                 — module declarations
    ├── map.rs                 — HashTable (open addressing)
    └── interner.rs            — Global StringInterner wrapper
tests/
└── tests.rs                   — Integration tests
```
---

## Building & Running

**Prerequisites:** Rust toolchain (edition 2024).

```bash
# build
cargo build

# run tests
cargo test

# run with debug disassembly output
cargo run
```

The disassembler prints annotated bytecode to stdout during compilation (enabled under `debug_assertions`):

```
=====Compile Successful=====
0000    1  OP_CONSTANT    0    42.01
0002    |  OP_CONSTANT    1    2
0004    |  OP_Add
...
```

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| [`string-interner`](https://crates.io/crates/string-interner) | `0.19.0` | Global string interning with `SymbolU32` handles |

---

## Known Limitations / Planned Work

- `OpCode::Not` is missing a disassembly branch (currently `todo!()` in `Chunk::disassemble_instruction`).
- `run-length encoding` for line number storage is not yet implemented (tracked in `todo.txt`).
- The REPL loop in `main.rs` is stubbed — `interpret()` calls `todo!()`.
- `read_string` in the VM uses `self.ip >= chunk.index_const24` as a heuristic to detect long constants, which is incorrect for some cases.
- no rehashing for `HashTable`.
- Runtime only garbage collection, garbage not collected during compilation.
- All strings are interned and owned by the string-interner. Therefore they cannot be garbage collected.
- Replace string-interner with our own Api, to allow string collection by gc.
