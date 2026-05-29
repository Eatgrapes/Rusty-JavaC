use crate::compiler::config::CompilerConfig;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub(crate) struct IncrementalBuild {
    newest_input: Option<SystemTime>,
}

impl IncrementalBuild {
    pub(crate) fn from_config(config: &CompilerConfig) -> Result<Option<Self>, Vec<String>> {
        if !config.incremental {
            return Ok(None);
        }

        let mut timestamps = InputTimestamps::default();
        for source_file in &config.source_files {
            timestamps.visit_required_file(Path::new(source_file));
        }
        for entry in classpath_entries(&config.classpath) {
            timestamps.visit_classpath_entry(&entry);
        }

        if timestamps.errors.is_empty() {
            Ok(Some(Self {
                newest_input: timestamps.newest,
            }))
        } else {
            Err(timestamps.errors)
        }
    }

    pub(crate) fn class_is_fresh(&self, class_file: &Path) -> bool {
        let Some(newest_input) = self.newest_input else {
            return false;
        };
        let Ok(metadata) = fs::metadata(class_file) else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }

        metadata
            .modified()
            .is_ok_and(|class_modified| class_modified >= newest_input)
    }
}

#[derive(Default)]
struct InputTimestamps {
    newest: Option<SystemTime>,
    errors: Vec<String>,
}

impl InputTimestamps {
    fn visit_required_file(&mut self, path: &Path) {
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => self.record_modified(path, &metadata),
            Ok(_) => self
                .errors
                .push(format!("source is not a file: {}", path.display())),
            Err(error) => self.errors.push(format!(
                "failed to read source metadata {}: {}",
                path.display(),
                error
            )),
        }
    }

    fn visit_classpath_entry(&mut self, path: &Path) {
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_dir() => {
                self.record_modified(path, &metadata);
                self.visit_classpath_directory(path);
            }
            Ok(metadata) if metadata.is_file() => self.record_modified(path, &metadata),
            Ok(_) => self.errors.push(format!(
                "classpath entry is not a file or directory: {}",
                path.display()
            )),
            Err(error) => self.errors.push(format!(
                "failed to read classpath metadata {}: {}",
                path.display(),
                error
            )),
        }
    }

    fn visit_classpath_directory(&mut self, directory: &Path) {
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
                Ok(entry) => self.visit_classpath_entry(&entry.path()),
                Err(error) => self.errors.push(format!(
                    "failed to read entry in {}: {}",
                    directory.display(),
                    error
                )),
            }
        }
    }

    fn record_modified(&mut self, path: &Path, metadata: &fs::Metadata) {
        match metadata.modified() {
            Ok(modified) => {
                self.newest = Some(self.newest.map_or(modified, |newest| newest.max(modified)));
            }
            Err(error) => self.errors.push(format!(
                "failed to read modification time for {}: {}",
                path.display(),
                error
            )),
        }
    }
}

fn classpath_entries(classpath: &[String]) -> Vec<PathBuf> {
    classpath
        .iter()
        .flat_map(|entry| std::env::split_paths(entry))
        .filter(|path| !path.as_os_str().is_empty())
        .collect()
}
