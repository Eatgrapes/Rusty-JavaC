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

    pub fn with_primary_label(mut self, message: impl Into<String>) -> Self {
        self.primary_label.message = message.into();
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

#[derive(Debug, Clone, Copy)]
pub struct SourceFile<'a> {
    pub name: &'a str,
    pub source: &'a str,
}

impl<'a> SourceFile<'a> {
    pub fn new(name: &'a str, source: &'a str) -> Self {
        Self { name, source }
    }
}

pub fn render_diagnostics(file: SourceFile<'_>, diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| render_diagnostic(file, diagnostic))
        .collect()
}

pub fn render_diagnostic(file: SourceFile<'_>, diagnostic: &Diagnostic) -> String {
    let primary = primary_position(file.source, diagnostic.primary_label.range);
    let header = diagnostic_header(diagnostic);
    let line_number_width = primary.line.to_string().len().max(1);
    let mut rendered = String::new();

    rendered.push_str(&format!("{header}: {}\n", diagnostic.message));
    rendered.push_str(&format!(
        "{:>width$}--> {}:{}:{}\n",
        "",
        file.name,
        primary.line,
        primary.column,
        width = line_number_width + 1
    ));
    rendered.push_str(&format!("{:>width$} |\n", "", width = line_number_width));
    rendered.push_str(&format!(
        "{:>width$} | {}\n",
        primary.line,
        primary.line_text,
        width = line_number_width
    ));
    rendered.push_str(&format!(
        "{:>width$} | {}{}",
        "",
        " ".repeat(primary.caret_start),
        "^".repeat(primary.caret_len.max(1)),
        width = line_number_width
    ));

    if !diagnostic.primary_label.message.is_empty() {
        rendered.push(' ');
        rendered.push_str(&diagnostic.primary_label.message);
    }
    rendered.push('\n');

    for label in &diagnostic.secondary_labels {
        let secondary = primary_position(file.source, label.range);
        rendered.push_str(&format!(
            "{:>width$} = note: {} at {}:{}\n",
            "",
            label.message,
            secondary.line,
            secondary.column,
            width = line_number_width
        ));
    }

    if let Some(help) = &diagnostic.help {
        rendered.push_str(&format!(
            "{:>width$} = help: {help}\n",
            "",
            width = line_number_width
        ));
    }

    rendered
}

fn diagnostic_header(diagnostic: &Diagnostic) -> String {
    let severity = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    };

    match &diagnostic.code {
        Some(code) => format!("{severity}[{code}]"),
        None => severity.to_string(),
    }
}

#[derive(Debug, Clone)]
struct PrimaryPosition {
    line: usize,
    column: usize,
    line_text: String,
    caret_start: usize,
    caret_len: usize,
}

fn primary_position(source: &str, range: TextRange) -> PrimaryPosition {
    let start = usize::try_from(u32::from(range.start()))
        .unwrap_or(0)
        .min(source.len());
    let end = usize::try_from(u32::from(range.end()))
        .unwrap_or(start)
        .min(source.len());
    let (line, line_start, line_end) = line_bounds(source, start);
    let column = source[line_start..start].chars().count() + 1;
    let line_text = source[line_start..line_end]
        .trim_end_matches(['\r', '\n'])
        .to_string();
    let caret_start = source[line_start..start].chars().count();
    let caret_end = if end <= line_end {
        source[line_start..end].chars().count()
    } else {
        source[line_start..line_end].chars().count()
    };

    PrimaryPosition {
        line,
        column,
        line_text,
        caret_start,
        caret_len: caret_end.saturating_sub(caret_start).max(1),
    }
}

fn line_bounds(source: &str, offset: usize) -> (usize, usize, usize) {
    let mut line = 1;
    let mut line_start = 0;

    for (index, ch) in source.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = index + 1;
        }
    }

    let line_end = source[line_start..]
        .find('\n')
        .map(|relative| line_start + relative)
        .unwrap_or(source.len());

    (line, line_start, line_end)
}
