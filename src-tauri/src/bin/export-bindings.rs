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
    let config = ts_rs::Config::default();
    println!("export {};", velo_engine::NameValue::decl(&config));
    println!("export {};", velo_engine::RequestMethod::decl(&config));
    println!("export {};", velo_engine::RequestBody::decl(&config));
    println!("export {};", velo_engine::ResourceBudget::decl(&config));
    println!("export {};", velo_engine::RampStage::decl(&config));
    println!("export {};", velo_engine::Workload::decl(&config));
    println!("export {};", velo_engine::RunConfig::decl(&config));
    println!("export {};", velo_engine::RunError::decl(&config));
    println!("export {};", velo_engine::RunConfirmation::decl(&config));
    println!("export {};", velo_engine::RunState::decl(&config));
    println!("export {};", velo_engine::Counters::decl(&config));
    println!("export {};", velo_engine::LatencySnapshot::decl(&config));
    println!("export {};", velo_engine::MetricsSnapshot::decl(&config));
    println!("export {};", velo_engine::RunSnapshot::decl(&config));
    println!("export {};", velo_engine::RunUpdate::decl(&config));
}
