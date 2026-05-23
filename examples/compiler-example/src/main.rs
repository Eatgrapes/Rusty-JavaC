use javac_compiler::config::CompilerConfig;
use javac_compiler::pipeline::compile;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: compiler-example [--output-dir <dir>] <source1.java> <source2.java> ...");
        std::process::exit(2);
    }

    let mut output_dir = "target/compiler-example".to_string();
    let mut sources: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output-dir" | "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --output-dir requires a value");
                    std::process::exit(2);
                }
                output_dir = args[i].clone();
                i += 1;
            }
            arg => {
                sources.push(arg.to_string());
                i += 1;
            }
        }
    }

    if sources.is_empty() {
        eprintln!("usage: compiler-example [--output-dir <dir>] <source1.java> <source2.java> ...");
        std::process::exit(2);
    }

    let mut config = CompilerConfig::new();
    config.output_dir = output_dir;
    config.source_files = sources;

    if let Err(errors) = compile(config) {
        for error in errors {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
}
