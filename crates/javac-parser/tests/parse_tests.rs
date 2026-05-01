use javac_ast::JavaLanguage;
use javac_parser::{Parse, Parser};
use rowan::SyntaxNode;

fn parse_file(name: &str) -> Parse {
    let path = format!("tests/java/{}", name);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
    Parser::parse(&source)
}

fn assert_no_errors(parse: &Parse, file: &str) {
    if !parse.errors.is_empty() {
        let msgs: Vec<String> = parse.errors.iter()
            .map(|e| format!("  offset {}: {}", e.offset, e.message))
            .collect();
        panic!("parse errors in {}:\n{}", file, msgs.join("\n"));
    }
}

#[test]
fn parse_hello_world() {
    let parse = parse_file("HelloWorld.java");
    assert_no_errors(&parse, "HelloWorld.java");
}

#[test]
fn parse_basic_types() {
    let parse = parse_file("BasicTypes.java");
    assert_no_errors(&parse, "BasicTypes.java");
}

#[test]
fn parse_control_flow() {
    let parse = parse_file("ControlFlow.java");
    assert_no_errors(&parse, "ControlFlow.java");
}

#[test]
fn parse_classes() {
    let parse = parse_file("Classes.java");
    assert_no_errors(&parse, "Classes.java");
}

#[test]
fn parse_java21_features() {
    let parse = parse_file("Java21Features.java");
    assert_no_errors(&parse, "Java21Features.java");
}

#[test]
fn parse_expressions() {
    let parse = parse_file("Expressions.java");
    assert_no_errors(&parse, "Expressions.java");
}

#[test]
fn parse_produces_green_tree() {
    let parse = parse_file("HelloWorld.java");
    let root = SyntaxNode::<JavaLanguage>::new_root(parse.green_node);
    let text = root.text().to_string();
    assert!(text.contains("Hello"), "green tree should contain source text");
    assert!(text.contains("World"), "green tree should contain source text");
}