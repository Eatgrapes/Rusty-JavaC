# Rusty-JavaC

> [!WARNING]
> This project is still under heavy development. It is not intended to replace the official Java compiler yet.

Rusty-JavaC is a Java compiler written in Rust.

## Why

The official Java compiler is written in Java. While this is great for self-hosting, it can be slow and memory-intensive in some cases. Rust provides several advantages for a compiler project:

- **Performance**: Rust's zero-cost abstractions and lack of a garbage collector allow for a faster compilation process.
- **Safety**: Rust's ownership model ensures memory safety and thread safety, reducing bugs in the compiler itself.
- **Modern Tooling**: Leveraging Cargo and the Rust ecosystem makes it easier to manage dependencies and build cross-platform binaries.

## Pipeline

The compiler is one Cargo package with stages split into modules under `src/`:

```text
Java source
  -> lexer
  -> parser
  -> ast
  -> hir lowering
  -> type and call resolution
  -> bytecode generation
  -> JVM .class file
```

## Use It

As a library:

```bash
cargo add rusty-javac
```

Or in `Cargo.toml`:

```toml
rusty-javac = "VERSION"
```

As the example compiler:

```bash
cargo run --example compiler-example -- --output-dir target/classes tests/java/HelloWorld.java
```

## Build

Requirements:

- Rust 1.85+
- cargo

```bash
git clone https://github.com/Eatgrapes/Rusty-JavaC.git
cd Rusty-JavaC
cargo build --locked
```

## Status

Some parts of Java are still incomplete. See [TODO.md](TODO.md) for the current backlog.

## Contributing

First Thanks!We welcome any PRs
Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR.

## License

[MIT](LICENSE)
