#[derive(Debug, Clone)]
pub enum IndentStyle {
    Tabs,
    Spaces,
}

#[derive(Debug, Clone)]
pub struct FormatterConfig {
    pub max_line_length: usize,
    pub indent_style: IndentStyle,
    pub indent_size: usize,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        FormatterConfig {
            max_line_length: 80,
            indent_style: IndentStyle::Tabs,
            indent_size: 1,
        }
    }
}

impl FormatterConfig {
    pub fn indent_str(&self) -> String {
        match self.indent_style {
            IndentStyle::Tabs => "\t".repeat(self.indent_size),
            IndentStyle::Spaces => " ".repeat(self.indent_size),
        }
    }

    pub fn indent_at_level(&self, level: usize) -> String {
        match self.indent_style {
            IndentStyle::Tabs => "\t".repeat(level * self.indent_size),
            IndentStyle::Spaces => " ".repeat(level * self.indent_size),
        }
    }
}
