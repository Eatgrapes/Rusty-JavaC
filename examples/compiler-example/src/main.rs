use javac_compiler::config::CompilerConfig;
use javac_compiler::pipeline::compile;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(source) = args.next() else {
        eprintln!("usage: compiler-example <source.java> [output-dir]");
        std::process::exit(2);
    };
    let output_dir = args
        .next()
        .unwrap_or_else(|| "target/compiler-example".to_string());

    let mut config = CompilerConfig::new();
    config.output_dir = output_dir;
    config.source_files.push(source);

    if let Err(errors) = compile(config) {
        for error in errors {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
}
