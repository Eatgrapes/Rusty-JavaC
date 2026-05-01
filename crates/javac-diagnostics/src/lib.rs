use text_size::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub message: String,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,
    pub primary_label: Label,
    pub secondary_labels: Vec<Label>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, range: TextRange) -> Self {
        Self {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            primary_label: Label {
                message: String::new(),
                range,
            },
            secondary_labels: Vec::new(),
            help: None,
        }
    }

    pub fn warning(message: impl Into<String>, range: TextRange) -> Self {
        Self {
            severity: Severity::Warning,
            code: None,
            message: message.into(),
            primary_label: Label {
                message: String::new(),
                range,
            },
            secondary_labels: Vec::new(),
            help: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_secondary(mut self, message: impl Into<String>, range: TextRange) -> Self {
        self.secondary_labels.push(Label {
            message: message.into(),
            range,
        });
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    pub fn is_ok(&self) -> bool {
        !self.items.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn items(&self) -> &[Diagnostic] {
        &self.items
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.items
    }
}

impl IntoIterator for Diagnostics {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

pub type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
