# Rusty-JavaC

> [!WARNING]
> This project is still under heavy development. It is not intended to replace the official Java compiler yet.

This is a project that rewrites the **Java compiler** using **Rust**.

## Why?

The official Java compiler is written in Java. While this is great for self-hosting, it can be slow and memory-intensive in some cases. Rust provides several advantages for a compiler project:

- **Performance**: Rust's zero-cost abstractions and lack of a garbage collector allow for a faster compilation process.
- **Safety**: Rust's ownership model ensures memory safety and thread safety, reducing bugs in the compiler itself.
- **Modern Tooling**: Leveraging Cargo and the Rust ecosystem makes it easier to manage dependencies and build cross-platform binaries.

## How?

Rusty-JavaC is built as a multi-stage compiler pipeline. Each stage is split into a separate crate, making the compiler easier to test, debug, and extend.

```text
Java source
   ↓
Lexer
   ↓
Parser
   ↓
AST / Syntax Tree
   ↓
HIR Lowering
   ↓
Type Analysis
   ↓
Call Resolution
   ↓
Bytecode Generation
   ↓
JVM .class File
```

## Library Goal

Rusty-JavaC is not only intended to be a standalone compiler. One of its long-term goals is to become a reusable Java compiler library for Rust projects.

In the future, Rusty-JavaC aims to provide stable APIs for tools such as Java analyzers, IDE plugins, code transformation tools, and custom Java-related compilers.

## Build

Requirements:

- **Rust** environment
- **Cargo**

```bash
git clone https://github.com/Eatgrapes/Rusty-JavaC.git

cd Rusty-JavaC

cargo build
```

## Contributing

This project is still young, and many parts of the compiler need improvement.

Contributions are welcome. You can help by improving the parser, adding Java syntax support, fixing lowering bugs, improving type analysis, working on bytecode generation, writing tests, or improving documentation.

If you are interested in compiler development, Java internals, JVM bytecode, or Rust, PRs are welcome.

## License

[MIT](LICENSE)