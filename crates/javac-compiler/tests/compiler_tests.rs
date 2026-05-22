use javac_compiler::config::CompilerConfig;
use javac_compiler::pipeline::compile;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn compiles_and_runs_println_with_string_concat() {
    let test_root = workspace_root()
        .join("target")
        .join("compiler-tests")
        .join("aaa");
    let source_dir = test_root.join("src");
    let output_dir = test_root.join("classes");
    let source_path = source_dir.join("AAA.java");

    let _ = std::fs::remove_dir_all(&test_root);
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::write(
        &source_path,
        r#"public class AAA {
    public static void main(String[] args) {
        int a = 10;
        int b = 33;
        int aa = a + b;
        System.out.println("Hello" + aa);
    }
}
"#,
    )
    .unwrap();

    let mut config = CompilerConfig::new();
    config.output_dir = output_dir.to_string_lossy().into_owned();
    config
        .source_files
        .push(source_path.to_string_lossy().into_owned());

    compile(config).unwrap_or_else(|errors| panic!("{}", errors.join("\n")));

    let output = Command::new("java")
        .arg("-Xverify:all")
        .arg("-cp")
        .arg(&output_dir)
        .arg("AAA")
        .output()
        .expect("failed to run generated AAA class");

    assert!(
        output.status.success(),
        "java failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        "Hello43"
    );
}
