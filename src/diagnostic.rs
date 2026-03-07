use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Position {
    pub const START: Self = Self {
        line: 1,
        column: 1,
        offset: 0,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedDiagnostic {
    pub source_index: usize,
    pub diagnostic: Diagnostic,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, position: Position) -> Self {
        Self {
            message: message.into(),
            position,
        }
    }

    pub fn render_with_source(&self, source: &str, label: Option<&str>) -> String {
        let mut output = String::new();
        output.push_str("error: ");
        output.push_str(&self.message);
        output.push('\n');
        output.push_str(" --> ");
        if let Some(label) = label {
            output.push_str(label);
            output.push(':');
        }
        output.push_str(&format!("{}:{}", self.position.line, self.position.column));

        if let Some(line) = source
            .split('\n')
            .nth(self.position.line.saturating_sub(1))
            .map(|line| line.trim_end_matches('\r'))
        {
            let gutter_width = self.position.line.to_string().len();
            output.push('\n');
            output.push_str(&format!("{:>gutter_width$} |\n", ""));
            output.push_str(&format!(
                "{:>gutter_width$} | {}\n",
                self.position.line, line
            ));
            output.push_str(&format!(
                "{:>gutter_width$} | {}^",
                "",
                " ".repeat(self.position.column.saturating_sub(1))
            ));
        }

        output
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.message, self.position.line, self.position.column
        )
    }
}

impl std::error::Error for Diagnostic {}

impl IndexedDiagnostic {
    pub fn new(source_index: usize, diagnostic: Diagnostic) -> Self {
        Self {
            source_index,
            diagnostic,
        }
    }
}
