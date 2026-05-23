# Contributing to Rusty-JavaC

First off, thanks for being interested in contributing! This project is still early-stage and there's a lot to do, so any help is appreciated — whether it's fixing a bug, adding tests, improving docs, or tackling a new piece of the compiler.

## Getting Started

### Prerequisites

You'll need:

- **Rust 1.85+** (we use the 2024 edition)
- **Cargo** (comes with Rust)
- **Java 21+** (for running the test `.class` files through `javap` and the JVM)
- **Git**

### Building

```bash
git clone https://github.com/Eatgrapes/Rusty-JavaC.git
cd Rusty-JavaC
cargo build --workspace
```

### Running the Tests

```bash
cargo test --workspace
```

This runs unit tests across all crates. If you've changed anything in the parser or bytecode generator, make sure these pass before opening a PR.

You can also do a quick end-to-end sanity check with the compiler example:

```bash
cargo run -p compiler-example -- tests/java/HelloWorld.java target/test-output
javap -v -c target/test-output/HelloWorld.class
```

This compiles a Java source file and lets you inspect the generated `.class` file.

## Project Structure

Rusty-JavaC is organized as a Cargo workspace. Each compiler stage lives in its own crate under `crates/`:

```
crates/
├── javac-ast           # Syntax node kinds and CST definitions (uses rowan)
├── javac-lexer         # Tokenizer (built on logos)
├── javac-parser        # Recursive-descent parser → CST
├── javac-hir           # High-level IR lowered from the CST
├── javac-ty            # Type analysis and checking
├── javac-call-resolver # Method/constructor overload resolution
├── javac-bytecode      # JVM bytecode instruction generation
├── javac-classfile     # .class file binary format writer
├── javac-diagnostics   # Error types and reporting
└── javac-compiler      # Ties everything together into a pipeline
```

The dependency flow is roughly linear:

```
lexer → parser → AST → HIR → ty → call-resolver → bytecode → classfile
                                                         ↘
                                                    diagnostics
```

`javac-compiler` depends on most of the above and exposes the top-level `compile()` API.

There's also:

- `examples/compiler-example` — a minimal CLI that shows how to use the compiler as a library.
- `tests/java/` — sample Java source files used for integration testing.

## What to Work On

Here are some areas where help is especially useful:

- **Parser coverage** — Lots of Java syntax is not yet supported. Pick a language feature (generics, annotations, lambdas, records, etc.) and add parsing for it.
- **HIR lowering** — The desugaring from CST to HIR is incomplete.
- **Type analysis** — Type inference and checking need work.
- **Bytecode generation** — We only handle a subset of the JVM instruction set. Control flow, exceptions, and invokedynamic are big missing pieces.
- **Tests** — More test Java files in `tests/java/` are always welcome. Even a simple `.java` file that exercises a specific feature helps a lot.
- **Diagnostics** — Better error messages, recovery from parse errors, suggestions.
- **Documentation** — Doc comments on public items, explanations of tricky parts, etc.

If you're not sure where to start, grep for `todo!()` or `unimplemented!()` — there are plenty.

## Development Workflow

1. **Fork and clone** the repo.
2. **Create a branch** from `master` for your changes. Use a descriptive name like `feat/parse-annotations` or `fix/missing-semicolon-recovery`.
3. **Make your changes.** Keep commits focused — one logical change per commit is ideal, but don't stress over it.
4. **Run `cargo build --workspace`** to make sure everything compiles.
5. **Run `cargo test --workspace`** to make sure nothing is broken.
6. **Open a pull request** against `master`.

## Code Style

We don't have a `rustfmt.toml` — just use the default `cargo fmt` settings. Before submitting:

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

A few general conventions used in the codebase:

- Public types and functions should have doc comments when it's not obvious what they do.
- Prefer returning `Result` types over panicking, especially in the pipeline crates.
- Keep crate boundaries clean. If you find yourself reaching across crate boundaries in weird ways, that's a sign something might need to be restructured — open an issue to discuss it.
- The project uses `thiserror` for error types. Stick with that pattern when adding new error variants.

## Commit Messages

We try to follow [Conventional Commits](https://www.conventionalcommits.org/). The format is:

```
type(scope): short description
```

**Types** we commonly use:

| Type | When to use |
|------|-------------|
| `feat` | A new feature or capability |
| `fix` | A bug fix |
| `refactor` | Code restructuring without behavior change |
| `docs` | Documentation only |
| `style` | Formatting, whitespace, no code logic change |
| `ci` | CI/workflow changes |
| `test` | Adding or updating tests |

**Scope** is usually the crate name without the `javac-` prefix — for example `compiler`, `hir`, `parser`, `bytecode`, `classfile`. You can leave it out for changes that span multiple crates or aren't crate-specific.

Some real examples from this repo:

```
feat(compiler): lower constructors and class fields
feat(hir): support package class names
fix(compiler): lower modern java control flow
refactor(hir): split lowering modules
docs: add project README
style: format workspace
ci: limit build workflow paths
```

Keep the subject line short (under ~72 chars) and lowercase. A message body is not required, but feel free to add one if the change needs extra context.

## Adding a New Test File

Drop a `.java` file into `tests/java/`. The file should be valid Java that compiles with `javac` — we compare our output against the reference compiler. Try to keep test files focused on one feature or edge case.

Good test file names describe what they're testing: `SwitchExpressionCases.java`, `GenericSignatureCase.java`, etc.

## CI

We have a GitHub Actions workflow (`.github/workflows/build.yml`) that runs on every PR touching `.rs` or `.toml` files. It:

1. Builds the entire workspace
2. Runs all tests
3. Compiles a test Java file end-to-end and verifies the `.class` file loads on a real JVM

Your PR needs to pass CI before it can be merged.

## Opening Issues

If you find a bug or have an idea but don't have time to implement it, feel free to open an issue. Including a small Java snippet that demonstrates the problem is really helpful.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE), same as the rest of the project.

---

Thanks again for contributing. Even small PRs make a difference at this stage of the project.
