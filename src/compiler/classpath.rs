#[path = "classpath/source.rs"]
mod source;

use self::source::JavaSource;
use crate::call_resolver::ClassCatalog;
use crate::ty::descriptor::{descriptor_to_ty, method_descriptor_to_sig};
use rust_asm::class_reader::read_class_file;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

const ACC_INTERFACE: u16 = 0x0200;

pub(crate) fn build_class_catalog(
    classpath: &[String],
    source_files: &[String],
) -> Result<ClassCatalog, Vec<String>> {
    let mut scanner = ClasspathScanner {
        catalog: ClassCatalog::platform(),
        errors: Vec::new(),
        sources: Vec::new(),
    };

    for source_file in source_files {
        scanner.register_primary_source(Path::new(source_file));
    }

    for entry in classpath_entries(classpath) {
        scanner.scan_classpath_entry(&entry);
    }

    scanner.register_source_members();

    if scanner.errors.is_empty() {
        Ok(scanner.catalog)
    } else {
        Err(scanner.errors)
    }
}

struct ClasspathScanner {
    catalog: ClassCatalog,
    errors: Vec<String>,
    sources: Vec<JavaSource>,
}

impl ClasspathScanner {
    fn register_primary_source(&mut self, path: &Path) {
        if let Ok(source) = fs::read_to_string(path) {
            self.register_java_source(path.display().to_string(), source);
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
            Ok(source) => self.register_java_source(path.display().to_string(), source),
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
                self.register_java_source(format!("{}!{}", path.display(), entry_name), source);
            }
        }
    }

    fn register_class_bytes(
        &mut self,
        label: &str,
        bytes: &[u8],
        fallback_internal_name: Option<String>,
    ) {
        match read_class_file(bytes) {
            Ok(class_file) => match class_file.class_name(class_file.this_class) {
                Ok(internal_name) => {
                    let internal_name = internal_name.to_string();
                    self.catalog.insert_internal_class(&internal_name);
                    if class_file.access_flags & ACC_INTERFACE != 0 {
                        self.catalog.mark_interface(&internal_name);
                    }
                    self.register_class_parents(&internal_name, &class_file);
                    self.register_class_members(&internal_name, &class_file);
                }
                Err(error) => self.handle_class_read_error(label, fallback_internal_name, error),
            },
            Err(error) => {
                self.handle_class_read_error(label, fallback_internal_name, error);
            }
        }
    }

    fn handle_class_read_error(
        &mut self,
        label: &str,
        fallback_internal_name: Option<String>,
        error: impl std::fmt::Display,
    ) {
        if let Some(internal_name) = fallback_internal_name {
            self.catalog.insert_internal_class(internal_name);
        } else {
            self.errors.push(format!(
                "failed to read class metadata from {label}: {error}"
            ));
        }
    }

    fn register_class_members(
        &mut self,
        internal_name: &str,
        class_file: &rust_asm::class_reader::ClassFile,
    ) {
        let is_interface = class_file.access_flags & ACC_INTERFACE != 0;
        for field in &class_file.fields {
            let Ok(name) = class_file.cp_utf8(field.name_index) else {
                continue;
            };
            let Ok(descriptor) = class_file.cp_utf8(field.descriptor_index) else {
                continue;
            };
            if let Some(ty) = descriptor_to_ty(descriptor) {
                self.catalog
                    .insert_field(internal_name, name, descriptor, ty, field.access_flags);
            }
        }

        for method in &class_file.methods {
            let Ok(name) = class_file.cp_utf8(method.name_index) else {
                continue;
            };
            let Ok(descriptor) = class_file.cp_utf8(method.descriptor_index) else {
                continue;
            };
            if let Some(sig) = method_descriptor_to_sig(name, descriptor) {
                self.catalog
                    .insert_method(internal_name, sig, method.access_flags, is_interface);
            }
        }
    }

    fn register_class_parents(
        &mut self,
        internal_name: &str,
        class_file: &rust_asm::class_reader::ClassFile,
    ) {
        if class_file.super_class != 0
            && let Ok(super_name) = class_file.class_name(class_file.super_class)
        {
            self.catalog.insert_parent(internal_name, super_name);
        }
        for interface in &class_file.interfaces {
            if let Ok(interface_name) = class_file.class_name(*interface) {
                self.catalog.insert_parent(internal_name, interface_name);
            }
        }
    }

    fn register_java_source(&mut self, label: String, source: String) {
        for internal_name in source::type_names(&source) {
            self.catalog.insert_internal_class(internal_name);
        }
        self.sources.push(JavaSource::new(label, source));
    }

    fn register_source_members(&mut self) {
        source::register_members(&mut self.catalog, &mut self.errors, &self.sources);
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
