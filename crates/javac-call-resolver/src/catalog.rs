use crate::platform;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ClassCatalog {
    classes: HashSet<String>,
    packages: HashSet<String>,
    simple_names: HashMap<String, SimpleName>,
}

#[derive(Debug, Clone)]
enum SimpleName {
    Unique(String),
    Ambiguous,
}

impl ClassCatalog {
    pub fn platform() -> Self {
        let mut catalog = Self::default();
        for class_name in platform::classes() {
            catalog.insert_internal_class(class_name);
        }
        catalog
    }

    pub fn insert_internal_class(&mut self, internal_name: impl AsRef<str>) {
        let Some(internal_name) = normalize_internal_name(internal_name.as_ref()) else {
            return;
        };
        if !self.classes.insert(internal_name.clone()) {
            return;
        }

        if let Some((package, simple_name)) = internal_name.rsplit_once('/') {
            self.packages.insert(package.to_string());
            self.insert_simple_name(simple_name, &internal_name);
        } else {
            self.insert_simple_name(&internal_name, &internal_name);
        }
    }

    pub fn contains_internal_class(&self, internal_name: &str) -> bool {
        self.classes.contains(internal_name)
    }

    pub fn contains_package(&self, package: &str) -> bool {
        self.packages.contains(package)
    }

    pub fn resolve_import(&self, path: &str, is_wildcard: bool) -> bool {
        let internal_name = path.replace('.', "/");
        if is_wildcard {
            self.contains_package(&internal_name)
        } else {
            self.contains_internal_class(&internal_name)
        }
    }

    pub fn resolve_qualified_name(&self, name: &str) -> Option<&str> {
        let internal_name = name.replace('.', "/");
        self.classes.get(internal_name.as_str()).map(String::as_str)
    }

    pub fn resolve_java_lang(&self, simple_name: &str) -> Option<&str> {
        let internal_name = format!("java/lang/{simple_name}");
        self.classes.get(internal_name.as_str()).map(String::as_str)
    }

    pub fn resolve_simple_name(&self, simple_name: &str) -> Option<&str> {
        match self.simple_names.get(simple_name)? {
            SimpleName::Unique(internal_name) => Some(internal_name.as_str()),
            SimpleName::Ambiguous => None,
        }
    }

    fn insert_simple_name(&mut self, simple_name: &str, internal_name: &str) {
        match self.simple_names.get(simple_name) {
            Some(SimpleName::Unique(existing)) if existing == internal_name => {}
            Some(_) => {
                self.simple_names
                    .insert(simple_name.to_string(), SimpleName::Ambiguous);
            }
            None => {
                self.simple_names.insert(
                    simple_name.to_string(),
                    SimpleName::Unique(internal_name.to_string()),
                );
            }
        }
    }
}

impl Default for ClassCatalog {
    fn default() -> Self {
        Self {
            classes: HashSet::new(),
            packages: HashSet::new(),
            simple_names: HashMap::new(),
        }
    }
}

fn normalize_internal_name(name: &str) -> Option<String> {
    let name = name.trim().trim_end_matches(".class");
    if name.is_empty() || name.starts_with('[') || name == "module-info" {
        return None;
    }
    Some(name.replace('.', "/"))
}
