# TODO

This file tracks known compiler work. A checkbox should be checked only when the feature exists in code and has fixture, parser, verifier, or equivalent coverage where applicable.

## Already Present

- [x] Single-package Rust project layout under `src/`
- [x] Public parser, AST, HIR, diagnostics, classfile, and compiler modules
- [x] `CompilerConfig` and `compile()` library entry point
- [x] Lexer, parser, HIR lowering, bytecode generation, and classfile writing pipeline
- [x] Cargo-style diagnostic renderer with source line, column, labels, and help text
- [x] Stable parser diagnostic code `P0001`
- [x] Lowering and bytecode diagnostic codes for current error families
- [x] Package declarations and package-based class output paths
- [x] Regular, static, and wildcard import parsing
- [x] Import resolution against platform classes, classpath classes, jars, directories, and Java source entries
- [x] Parser support for class, interface, enum, record, and annotation declarations
- [x] Parser support for sealed, non-sealed, and permits syntax
- [x] Parser support for annotations on modifier lists
- [x] Parser support for generic type parameters, generic type arguments, wildcards, and varargs syntax
- [x] Parser support for nested member type syntax
- [x] Parser support for compact record constructor syntax
- [x] Parser support for if/else, for, enhanced for, while, do/while, switch, break, continue, return, yield, throw, assert, synchronized, labeled statements, and try/catch/finally
- [x] Parser support for try-with-resources
- [x] Parser fixtures that parse every root `tests/java/*.java` file
- [x] `javac --release 21` acceptance check for every root Java fixture
- [x] HIR model for classes, fields, methods, constructors, imports, packages, statements, expressions, lambdas, anonymous classes, try statements, assertions, synchronized statements, and labels
- [x] HIR lowering for supported class fields, methods, constructors, local variables, loops, switches, labels, break/continue, throw, try/catch/finally, try-with-resources, and array enhanced-for loops
- [x] Basic `var` local type inference from initializers
- [x] Pattern variable binding for direct `if (x instanceof T name)` true branches
- [x] Shared expression type inference used by lowering, validation, and bytecode generation
- [x] Generic Signature attributes for supported generic class and method declarations
- [x] SourceFile, LineNumberTable, LocalVariableTable, StackMapTable, and Signature emission for supported fixtures
- [x] Classpath scanning for directories, jars, `.class` files, and `.java` files
- [x] Classpath `.class` metadata registration for class names, fields, methods, access flags, superclass, and interfaces
- [x] Source classpath metadata registration for class, interface, method, field, parent, and varargs information
- [x] ClassCatalog lookup through superclass/interface chains
- [x] Basic method overload scoring with primitive widening, reference assignability, boxing, unboxing, and varargs
- [x] Platform signatures for common `java.lang`, `java.util`, `java.io`, `java.net`, `java.nio.file`, `java.time`, and `java.util.function` classes
- [x] Lambda lowering through `invokedynamic` and `LambdaMetafactory`
- [x] Functional interface method lookup from `ClassCatalog` for supported SAM interfaces
- [x] Anonymous class generation for supported fixtures
- [x] NestHost and NestMembers metadata for supported anonymous class output
- [x] Bytecode generation for supported int, string, and enum-style switch cases
- [x] Bytecode generation for try/catch/finally and try-with-resources in supported shapes
- [x] Bytecode exception table emission for supported try/catch/finally/resources code
- [x] Direct array allocation and initializer bytecode for supported array expressions
- [x] Assignment expression/effect codegen modes that avoid unnecessary `dup`/`pop` for supported assignments
- [x] Bytecode validation for unresolved variables, fields, method calls, and some invalid receivers
- [x] Incremental compile timestamp scaffold
- [x] CI path filters that skip docs-only changes unless manually triggered

## Parser Remaining

- [ ] Parse annotation element declarations completely
- [ ] Parse annotation arguments as Java element values, including named arguments and array values
- [ ] Parse module-info.java
- [ ] Parse local class declarations in method bodies
- [ ] Parse method references such as `String::valueOf`
- [ ] Parse multi-catch union types
- [ ] Parse receiver parameters completely
- [ ] Parse switch pattern labels and guarded patterns
- [ ] Add stronger parser recovery that keeps useful later errors
- [ ] Add focused parser fixtures for every supported declaration and statement form

## HIR And Lowering Remaining

- [ ] Split `src/hir/lowering/expr.rs` by expression family
- [ ] Split `src/hir/lowering/stmt.rs` by statement family
- [ ] Move binding decisions out of syntax-shaped lowering code
- [ ] Preserve precise source spans for every lowered expression and statement
- [ ] Preserve annotations in HIR
- [ ] Lower top-level interfaces into real HIR type declarations
- [ ] Lower top-level enum declarations into JVM-compatible classes
- [ ] Lower top-level records into fields, accessors, constructor, and standard methods
- [ ] Lower top-level annotation declarations
- [ ] Lower multiple top-level source declarations instead of requiring one top-level class
- [ ] Lower nested member classes
- [ ] Lower local classes
- [ ] Lower method references
- [ ] Lower synchronized blocks and methods into codegen-ready HIR
- [ ] Lower enhanced for loops over `Iterable`
- [ ] Lower captured locals for lambdas and inner classes
- [ ] Track pattern variable scope through boolean control flow
- [ ] Track definite assignment before bytecode generation
- [ ] Represent typed HIR expressions after inference/resolution

## Type System Remaining

- [ ] Implement generic type substitution for fields and methods
- [ ] Implement generic method inference
- [ ] Implement wildcard capture conversion
- [ ] Complete numeric promotion and constant narrowing rules
- [ ] Complete boxing and unboxing conversion coverage
- [ ] Implement javac-style overload resolution phases
- [ ] Resolve default interface methods with Java-compatible conflict rules
- [ ] Resolve bridge methods from classfile metadata when needed
- [ ] Track access checks for public/protected/private/package-private members
- [ ] Track static-vs-instance misuse comprehensively
- [ ] Validate checked exceptions
- [ ] Validate unreachable statements
- [ ] Validate definite return for non-void methods
- [ ] Validate final fields and final locals
- [ ] Validate sealed class inheritance rules
- [ ] Validate annotation target/value rules
- [ ] Validate enum and record language rules

## Classpath And Platform Remaining

- [ ] Read generic Signature attributes from classpath classes
- [ ] Store generic field signatures from classpath `.class` files
- [ ] Store generic method signatures from classpath `.class` files
- [ ] Resolve nested classes from InnerClasses attributes
- [ ] Resolve EnclosingMethod metadata for local and anonymous classes
- [ ] Compile imported or referenced source dependencies in dependency order
- [ ] Detect duplicate classes across source and classpath entries
- [ ] Detect ambiguous wildcard imports
- [ ] Support sourcepath separately from classpath
- [ ] Cache classpath scans across incremental compiles
- [ ] Add module path support
- [ ] Expand platform signatures without turning them into ad hoc call hardcoding
- [ ] Add platform coverage for collection, stream, regex, concurrency, reflection, and annotation APIs

## Bytecode Generation Remaining

- [ ] Emit RuntimeVisibleAnnotations and RuntimeInvisibleAnnotations
- [ ] Emit InnerClasses attributes for member, local, and anonymous classes
- [ ] Emit EnclosingMethod attributes for local and anonymous classes
- [ ] Finish complete NestHost and NestMembers metadata for every nested class shape
- [ ] Emit monitorenter/monitorexit for synchronized code
- [ ] Emit bootstrap metadata for method references
- [ ] Replace linear string switch checks with efficient javac-style string switch bytecode
- [ ] Generate enum switch mapping compatible with Java semantics
- [ ] Complete try-with-resources suppressed-exception behavior for mixed try/catch/finally/resource shapes
- [ ] Verify StackMapTable generation for all loop, branch, switch, and exception-handler shapes
- [ ] Verify LineNumberTable output for multi-line expressions
- [ ] Verify LocalVariableTable slot lifetimes for shadowing and nested scopes
- [ ] Remove temporary locals introduced only for codegen convenience
- [ ] Compare generated bytecode against `javac` for representative fixtures
- [ ] Add verifier tests for every fixture with a runnable `main`

## Diagnostics Remaining

- [ ] Give every lowering error a specific stable diagnostic code
- [ ] Give every bytecode validation error a specific stable diagnostic code
- [ ] Give every lowering error a precise source span
- [ ] Give every semantic error a precise source span
- [ ] Report multiple independent errors inside one source file
- [ ] Distinguish syntax errors from unsupported language features
- [ ] Add suggestions for common import typos
- [ ] Add suggestions for missing classpath entries
- [ ] Add suggestions for method overload mismatches
- [ ] Add suggestions for static-vs-instance misuse
- [ ] Use snapshot tests for rendered diagnostics
- [ ] Keep diagnostic tests focused on stable output contracts

## Tests And Fixtures Remaining

- [ ] Add a Rust fixture runner that compiles all root `tests/java/*.java` files with Rusty-JavaC
- [ ] Add reusable `javap -v` smoke checks
- [ ] Add reusable JVM verifier checks
- [ ] Add `javac` bytecode/runtime comparison tests for accepted fixtures
- [ ] Add negative compile fixtures for parser errors
- [ ] Add negative compile fixtures for missing symbols
- [ ] Add negative compile fixtures for type mismatches
- [ ] Add negative compile fixtures for invalid imports and classpath entries
- [ ] Add snapshot tests for diagnostics
- [ ] Add real-world classpath fixture using ASM
- [ ] Add fixtures for Java collections, streams, regex, time, file IO, networking, reflection, and annotations
- [ ] Add fixtures for packages with cross-file references
- [ ] Add fixtures for nested classes, local classes, and member classes
- [ ] Add fixtures for exception handling edge cases
- [ ] Add fixtures for synchronized code
- [ ] Add fuzz tests for lexer/parser stability

## Public API And Tooling Remaining

- [ ] Document the library API with rustdoc examples
- [ ] Expose a structured compile result instead of only rendered error strings
- [ ] Stabilize the parser/AST API for tools that do not need bytecode
- [ ] Stabilize the HIR API for analysis tools
- [ ] Add a stable API for custom classpath providers
- [ ] Add a stable API for incremental compilation state
- [ ] Add benchmark coverage against `javac` on small and medium inputs
- [ ] Add a release checklist for crates.io publishing
- [ ] Keep the package published as one `rusty-javac` crate
