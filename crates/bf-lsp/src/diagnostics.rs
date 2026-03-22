use tower_lsp::lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticSeverity, NumberOrString, Position as LspPosition,
    Range,
};

use bf_core::validator::{validate, Severity};

pub fn to_lsp_diagnostics(source: &str) -> Vec<LspDiagnostic> {
    validate(source)
        .into_iter()
        .map(|d| {
            let line = d.position.line.saturating_sub(1) as u32;
            let col = d.position.column.saturating_sub(1) as u32;
            LspDiagnostic {
                range: Range {
                    start: LspPosition::new(line, col),
                    end: LspPosition::new(line, col + 1),
                },
                severity: Some(match d.severity {
                    Severity::Error => DiagnosticSeverity::ERROR,
                    Severity::Warning => DiagnosticSeverity::WARNING,
                    Severity::Info => DiagnosticSeverity::INFORMATION,
                }),
                code: Some(NumberOrString::String(d.rule)),
                source: Some("bf-lsp".to_string()),
                message: d.message,
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmatched_open_bracket_is_error() {
        let diags = to_lsp_diagnostics("[+");
        assert!(diags.iter().any(|d| matches!(d.severity, Some(DiagnosticSeverity::ERROR))));
        assert!(diags.iter().any(|d| d.source == Some("bf-lsp".to_string())));
    }

    #[test]
    fn unmatched_close_bracket_is_error() {
        let diags = to_lsp_diagnostics("+]");
        assert!(diags.iter().any(|d| matches!(d.severity, Some(DiagnosticSeverity::ERROR))));
    }

    #[test]
    fn empty_loop_is_warning() {
        let diags = to_lsp_diagnostics("[]");
        assert!(diags.iter().any(|d| matches!(d.severity, Some(DiagnosticSeverity::WARNING))));
    }

    #[test]
    fn plus_minus_is_info() {
        let diags = to_lsp_diagnostics("+-");
        assert!(diags.iter().any(|d| matches!(d.severity, Some(DiagnosticSeverity::INFORMATION))));
    }

    #[test]
    fn clean_program_no_diagnostics() {
        let diags = to_lsp_diagnostics(">+[-<+>]");
        assert!(diags.is_empty());
    }

    #[test]
    fn position_is_zero_based() {
        // "[" at source position (1,1) → LSP (0,0)
        let diags = to_lsp_diagnostics("[+");
        let bf002 = diags
            .iter()
            .find(|d| matches!(&d.code, Some(NumberOrString::String(s)) if s == "BF002"))
            .expect("expected BF002 diagnostic");
        assert_eq!(bf002.range.start.line, 0);
        assert_eq!(bf002.range.start.character, 0);
    }
}
