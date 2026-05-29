use rowan::SyntaxNode;
use rusty_javac::ast::{JavaLanguage, JavaSyntaxKind};
use rusty_javac::parser::{Parse, Parser};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn fixtures_dir() -> PathBuf {
    workspace_root().join("tests").join("java")
}

fn java_fixtures() -> Vec<PathBuf> {
    let mut fixtures = std::fs::read_dir(fixtures_dir())
        .expect("failed to read root tests/java directory")
        .map(|entry| entry.expect("failed to read java fixture entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
        .collect::<Vec<_>>();
    fixtures.sort();
    assert!(!fixtures.is_empty(), "expected at least one Java fixture");
    fixtures
}

fn assert_no_errors(parse: &Parse, file: &Path) {
    if !parse.errors.is_empty() {
        let msgs: Vec<String> = parse
            .errors
            .iter()
            .map(|e| format!("  offset {}: {}", e.offset, e.message))
            .collect();
        panic!("parse errors in {}:\n{}", file.display(), msgs.join("\n"));
    }
}

#[test]
fn parse_all_java_fixtures() {
    for path in java_fixtures() {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
        let parse = Parser::parse(&source);
        assert_no_errors(&parse, &path);
    }
}

#[test]
fn parser_builds_green_tree_for_all_java_fixtures() {
    for path in java_fixtures() {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
        let parse = Parser::parse(&source);
        assert_no_errors(&parse, &path);
        let root = SyntaxNode::<JavaLanguage>::new_root(parse.green_node);
        assert_eq!(root.kind(), JavaSyntaxKind::CompilationUnit);
        assert!(
            !root.text().to_string().is_empty(),
            "green tree should contain parsed tokens for {}",
            path.display()
        );
    }
}

#[test]
fn javac_accepts_all_java_fixtures() {
    let output_root = workspace_root().join("target").join("java-fixture-classes");
    std::fs::create_dir_all(&output_root)
        .unwrap_or_else(|e| panic!("failed to create {}: {}", output_root.display(), e));

    for path in java_fixtures() {
        let fixture_output = output_root.join(
            path.file_stem()
                .expect("java fixture should have a file stem"),
        );
        let _ = std::fs::remove_dir_all(&fixture_output);
        std::fs::create_dir_all(&fixture_output)
            .unwrap_or_else(|e| panic!("failed to create {}: {}", fixture_output.display(), e));

        let output = Command::new("javac")
            .arg("--release")
            .arg("21")
            .arg("-d")
            .arg(&fixture_output)
            .arg(&path)
            .output()
            .unwrap_or_else(|e| panic!("failed to run javac for {}: {}", path.display(), e));

        assert!(
            output.status.success(),
            "javac failed for {}\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
