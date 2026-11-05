use velo_contracts::{CommandError, DiagnosticInput, DiagnosticReport};

#[tauri::command]
pub(crate) fn diagnose(input: DiagnosticInput) -> Result<DiagnosticReport, CommandError> {
    let length = input.label.trim().chars().count();
    if !(1..=64).contains(&length) {
        return Err(CommandError {
            code: "INVALID_DIAGNOSTIC_LABEL".into(),
            message: "Enter a label between 1 and 64 characters.".into(),
            field: Some("label".into()),
        });
    }
    let (engine, version) = velo_engine::identity();
    tracing::info!(engine, "Native diagnostic completed");
    Ok(DiagnosticReport {
        engine: engine.into(),
        version: version.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_uses_engine_identity() {
        let report = diagnose(DiagnosticInput {
            label: "desktop".into(),
        })
        .unwrap();
        assert_eq!(report.engine, velo_engine::identity().0);
        assert_eq!(
            serde_json::to_value(report).unwrap()["version"],
            velo_engine::identity().1
        );
    }

    #[test]
    fn invalid_labels_have_safe_structured_errors() {
        for label in [" ".to_string(), "a".repeat(65)] {
            let error = diagnose(DiagnosticInput { label }).unwrap_err();
            let value = serde_json::to_value(error).unwrap();
            assert_eq!(value["code"], "INVALID_DIAGNOSTIC_LABEL");
            assert_eq!(value["field"], "label");
        }
        assert!(
            diagnose(DiagnosticInput {
                label: "é".repeat(64)
            })
            .is_ok()
        );
    }
}
