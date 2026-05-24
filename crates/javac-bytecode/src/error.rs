#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct BytecodeError {
    pub code: &'static str,
    pub message: String,
    pub line: Option<u16>,
    pub needle: Option<String>,
    pub label: Option<String>,
    pub help: Option<String>,
}

impl BytecodeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            code: "B0001",
            message: message.into(),
            line: None,
            needle: None,
            label: None,
            help: None,
        }
    }

    pub fn at_line(message: impl Into<String>, line: Option<u16>) -> Self {
        Self {
            code: "B0001",
            message: message.into(),
            line,
            needle: None,
            label: None,
            help: None,
        }
    }

    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = code;
        self
    }

    pub fn with_needle(mut self, needle: impl Into<String>) -> Self {
        self.needle = Some(needle.into());
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

impl From<String> for BytecodeError {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for BytecodeError {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}
