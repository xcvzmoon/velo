#[allow(
    dead_code,
    reason = "Binding exporter shares DTO definitions with the command module"
)]
#[path = "../diagnostic.rs"]
mod diagnostic;
use ts_rs::TS;

fn main() {
    println!(
        "export {};\nexport {};\nexport {};",
        diagnostic::DiagnosticInput::decl(&ts_rs::Config::default()),
        diagnostic::DiagnosticReport::decl(&ts_rs::Config::default()),
        diagnostic::CommandError::decl(&ts_rs::Config::default())
    );
}
