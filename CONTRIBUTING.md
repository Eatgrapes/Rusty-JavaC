# Contributing to Rusty-JavaC

Rusty-JavaC is still early-stage. Focused fixes, tests, docs, and compiler feature work are all useful.

## Getting Started

### Prerequisites

- Rust 1.85+ with Cargo
- Java 21+ for `javap` and JVM verification
- Git

### Building

```bash
git clone https://github.com/Eatgrapes/Rusty-JavaC.git
cd Rusty-JavaC
cargo build --workspace
```

### Running Tests

```bash
cargo test --workspace
```

For parser or bytecode changes, also run the compiler example and inspect the generated class:

```bash
cargo run --example compiler-example -- --output-dir target/test-output tests/java/HelloWorld.java
javap -v -c target/test-output/HelloWorld.class
```

## Project Structure

Rusty-JavaC is a single Cargo package. Compiler stages live as regular Rust modules under `src/`:

```text
src/
|- ast           # Syntax node kinds and CST helpers
|- lexer         # Tokenizer built on logos
|- parser        # Recursive-descent parser to CST
|- hir           # High-level IR and lowering from CST
|- ty            # Type model, descriptors, and assignability checks
|- call_resolver # Class catalog and method/constructor resolution
|- bytecode      # JVM bytecode generation and validation
|- classfile     # .class binary reader/writer helpers
|- diagnostics   # Error rendering
`- compiler      # Classpath, incremental state, and compile pipeline
```

The rough data flow is:

```text
lexer -> parser -> ast -> hir -> ty -> call_resolver -> bytecode -> classfile
                                      \-> diagnostics
```

`src/lib.rs` only declares the top-level modules and re-exports `CompilerConfig` plus `compile()`.

There is also:

- `examples/compiler-example.rs`: a minimal CLI showing library usage.
- `tests/java/`: Java source fixtures used for integration checks.

## What To Work On

- Parser coverage: Java syntax is still incomplete.
- HIR lowering: many Java constructs need more complete lowering.
- Type analysis: inference and checking need more cases.
- Bytecode generation: exceptions, control flow edges, and JVM metadata need care.
- Diagnostics: better messages, recovery, and suggestions.
- Tests: focused `.java` fixtures in `tests/java/`.

## Development Workflow

1. Create a branch from `master`.
2. Keep changes focused.
3. Run `cargo fmt --all`.
4. Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
5. Run `cargo test --workspace`.
6. For bytecode changes, compile relevant Java fixtures and inspect with `javap`.

## Code Style

Use default `rustfmt`. Keep modules cohesive and prefer explicit module paths over broad root re-exports. Public items should have docs when behavior is not obvious.

Return `Result` for recoverable failures. Use `thiserror` for error types when adding new compiler errors.

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```text
type(scope): short description
```

Common types:

| Type | When to use |
|------|-------------|
| `feat` | New capability |
| `fix` | Bug fix |
| `refactor` | Structure change without intended behavior change |
| `docs` | Documentation only |
| `style` | Formatting or whitespace |
| `ci` | Workflow changes |
| `test` | Test changes |

Good scopes are module names such as `compiler`, `hir`, `parser`, `bytecode`, and `classfile`.

## Adding Java Fixtures

Put `.java` files under `tests/java/`. They should compile with `javac` and focus on a feature or edge case.

## CI

`.github/workflows/build.yml` builds the package, runs tests, compiles a Java file end-to-end, and verifies the generated class on the JVM.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
