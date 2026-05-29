use rusty_javac::{CompilerConfig, compile};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        std::process::exit(2);
    }

    let mut output_dir = "target/compiler-example".to_string();
    let mut classpath: Vec<String> = Vec::new();
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
            "--class-path" | "--classpath" | "-classpath" | "-cp" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: {} requires a value", args[i - 1]);
                    std::process::exit(2);
                }
                classpath.push(args[i].clone());
                i += 1;
            }
            arg => {
                sources.push(arg.to_string());
                i += 1;
            }
        }
    }

    if sources.is_empty() {
        print_usage();
        std::process::exit(2);
    }

    let mut config = CompilerConfig::new();
    config.output_dir = output_dir;
    config.classpath = classpath;
    config.source_files = sources;

    if let Err(errors) = compile(config) {
        for error in errors {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
}

fn print_usage() {
    eprintln!(
        "usage: compiler-example [--output-dir <dir>] [--class-path <path>] <source1.java> <source2.java> ..."
    );
}
