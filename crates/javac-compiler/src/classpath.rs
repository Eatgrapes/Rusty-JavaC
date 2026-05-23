use javac_ast::JavaSyntaxKind;
use javac_call_resolver::ClassCatalog;
use javac_lexer::Lexer;
use rust_asm::class_reader::read_class_file;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

pub(crate) fn build_class_catalog(
    classpath: &[String],
    source_files: &[String],
) -> Result<ClassCatalog, Vec<String>> {
    let mut scanner = ClasspathScanner {
        catalog: ClassCatalog::platform(),
        errors: Vec::new(),
    };

    for source_file in source_files {
        scanner.register_primary_source(Path::new(source_file));
    }

    for entry in classpath_entries(classpath) {
        scanner.scan_classpath_entry(&entry);
    }

    if scanner.errors.is_empty() {
        Ok(scanner.catalog)
    } else {
        Err(scanner.errors)
    }
}

struct ClasspathScanner {
    catalog: ClassCatalog,
    errors: Vec<String>,
}

impl ClasspathScanner {
    fn register_primary_source(&mut self, path: &Path) {
        if let Ok(source) = fs::read_to_string(path) {
            self.register_java_source(&source);
        }
    }

    fn scan_classpath_entry(&mut self, path: &Path) {
        if !path.exists() {
            self.errors
                .push(format!("classpath entry not found: {}", path.display()));
            return;
        }

        if path.is_dir() {
            self.scan_directory(path, path);
        } else {
            self.scan_file(path, None);
        }
    }

    fn scan_directory(&mut self, root: &Path, directory: &Path) {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) => {
                self.errors.push(format!(
                    "failed to read classpath directory {}: {}",
                    directory.display(),
                    error
                ));
                return;
            }
        };

        for entry in entries {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.is_dir() {
                        self.scan_directory(root, &path);
                    } else {
                        let fallback = fallback_internal_name(root, &path);
                        self.scan_file(&path, fallback);
                    }
                }
                Err(error) => self.errors.push(format!(
                    "failed to read entry in {}: {}",
                    directory.display(),
                    error
                )),
            }
        }
    }

    fn scan_file(&mut self, path: &Path, fallback_internal_name: Option<String>) {
        match extension(path).as_deref() {
            Some("class") => self.register_class_file(path, fallback_internal_name),
            Some("jar") => self.scan_jar(path),
            Some("java") => self.register_classpath_source(path),
            _ => {}
        }
    }

    fn register_classpath_source(&mut self, path: &Path) {
        match fs::read_to_string(path) {
            Ok(source) => self.register_java_source(&source),
            Err(error) => self.errors.push(format!(
                "failed to read classpath source {}: {}",
                path.display(),
                error
            )),
        }
    }

    fn register_class_file(&mut self, path: &Path, fallback_internal_name: Option<String>) {
        match fs::read(path) {
            Ok(bytes) => self.register_class_bytes(
                &path.display().to_string(),
                &bytes,
                fallback_internal_name,
            ),
            Err(error) => self.errors.push(format!(
                "failed to read class file {}: {}",
                path.display(),
                error
            )),
        }
    }

    fn scan_jar(&mut self, path: &Path) {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) => {
                self.errors
                    .push(format!("failed to open jar {}: {}", path.display(), error));
                return;
            }
        };
        let mut archive = match ZipArchive::new(file) {
            Ok(archive) => archive,
            Err(error) => {
                self.errors
                    .push(format!("failed to read jar {}: {}", path.display(), error));
                return;
            }
        };

        for index in 0..archive.len() {
            let mut entry = match archive.by_index(index) {
                Ok(entry) => entry,
                Err(error) => {
                    self.errors.push(format!(
                        "failed to read entry {} from jar {}: {}",
                        index,
                        path.display(),
                        error
                    ));
                    continue;
                }
            };
            let entry_name = entry.name().replace('\\', "/");
            if entry_name.ends_with(".class") {
                let mut bytes = Vec::new();
                if let Err(error) = entry.read_to_end(&mut bytes) {
                    self.errors.push(format!(
                        "failed to read class {} from jar {}: {}",
                        entry_name,
                        path.display(),
                        error
                    ));
                    continue;
                }
                let fallback = entry_name.strip_suffix(".class").map(str::to_string);
                self.register_class_bytes(
                    &format!("{}!{}", path.display(), entry_name),
                    &bytes,
                    fallback,
                );
            } else if entry_name.ends_with(".java") {
                let mut source = String::new();
                if let Err(error) = entry.read_to_string(&mut source) {
                    self.errors.push(format!(
                        "failed to read source {} from jar {}: {}",
                        entry_name,
                        path.display(),
                        error
                    ));
                    continue;
                }
                self.register_java_source(&source);
            }
        }
    }

    fn register_class_bytes(
        &mut self,
        label: &str,
        bytes: &[u8],
        fallback_internal_name: Option<String>,
    ) {
        match read_class_file(bytes).and_then(|class_file| {
            class_file
                .class_name(class_file.this_class)
                .map(str::to_string)
        }) {
            Ok(internal_name) => self.catalog.insert_internal_class(internal_name),
            Err(error) => {
                if let Some(internal_name) = fallback_internal_name {
                    self.catalog.insert_internal_class(internal_name);
                } else {
                    self.errors.push(format!(
                        "failed to read class metadata from {label}: {error}"
                    ));
                }
            }
        }
    }

    fn register_java_source(&mut self, source: &str) {
        for internal_name in source_type_names(source) {
            self.catalog.insert_internal_class(internal_name);
        }
    }
}

#[derive(Debug, Clone)]
struct Token {
    kind: JavaSyntaxKind,
    text: String,
}

fn source_type_names(source: &str) -> Vec<String> {
    let tokens = Lexer::new(source)
        .map(|token| Token {
            kind: token.kind,
            text: token.text,
        })
        .collect::<Vec<_>>();
    let package = package_name(&tokens);
    let mut names = Vec::new();
    let mut depth = 0usize;
    let mut i = 0usize;

    while i < tokens.len() {
        match tokens[i].kind {
            JavaSyntaxKind::LBrace => {
                depth += 1;
                i += 1;
            }
            JavaSyntaxKind::RBrace => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            kind if depth == 0 && is_type_keyword(kind) => {
                if let Some(simple_name) = next_ident(&tokens, i + 1) {
                    names.push(internal_name(package.as_deref(), simple_name));
                }
                i += 1;
            }
            JavaSyntaxKind::At
                if depth == 0
                    && tokens
                        .get(i + 1)
                        .is_some_and(|token| token.kind == JavaSyntaxKind::InterfaceKw) =>
            {
                if let Some(simple_name) = next_ident(&tokens, i + 2) {
                    names.push(internal_name(package.as_deref(), simple_name));
                }
                i += 3;
            }
            _ => i += 1,
        }
    }

    names
}

fn package_name(tokens: &[Token]) -> Option<String> {
    let package_index = tokens
        .iter()
        .position(|token| token.kind == JavaSyntaxKind::PackageKw)?;
    let (package, _) = qualified_name(tokens, package_index + 1)?;
    Some(package)
}

fn qualified_name(tokens: &[Token], start: usize) -> Option<(String, usize)> {
    let mut name = String::new();
    let mut i = start;
    let mut expecting_ident = true;

    while let Some(token) = tokens.get(i) {
        match token.kind {
            JavaSyntaxKind::Ident if expecting_ident => {
                name.push_str(&token.text);
                expecting_ident = false;
            }
            JavaSyntaxKind::Dot if !expecting_ident => {
                name.push('.');
                expecting_ident = true;
            }
            _ => break,
        }
        i += 1;
    }

    (!name.is_empty() && !expecting_ident).then_some((name, i))
}

fn next_ident(tokens: &[Token], start: usize) -> Option<&str> {
    tokens
        .iter()
        .skip(start)
        .find(|token| token.kind == JavaSyntaxKind::Ident)
        .map(|token| token.text.as_str())
}

fn is_type_keyword(kind: JavaSyntaxKind) -> bool {
    matches!(
        kind,
        JavaSyntaxKind::ClassKw
            | JavaSyntaxKind::InterfaceKw
            | JavaSyntaxKind::EnumKw
            | JavaSyntaxKind::RecordKw
    )
}

fn internal_name(package: Option<&str>, simple_name: &str) -> String {
    match package {
        Some(package) => format!("{}/{}", package.replace('.', "/"), simple_name),
        None => simple_name.to_string(),
    }
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

fn fallback_internal_name(root: &Path, path: &Path) -> Option<String> {
    if extension(path).as_deref() != Some("class") {
        return None;
    }

    let relative_path = path.strip_prefix(root).ok()?;
    let mut internal_name = PathBuf::new();
    internal_name.push(relative_path);
    internal_name.set_extension("");
    Some(path_to_internal_name(&internal_name))
}

fn path_to_internal_name(path: &Path) -> String {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

fn classpath_entries(classpath: &[String]) -> Vec<PathBuf> {
    classpath
        .iter()
        .flat_map(|entry| std::env::split_paths(entry))
        .filter(|path| !path.as_os_str().is_empty())
        .collect()
}
