use ts_rs::TS;
use velo_contracts::{CommandError, DiagnosticInput, DiagnosticReport};

fn main() {
    println!(
        "export {};\nexport {};\nexport {};",
        DiagnosticInput::decl(&ts_rs::Config::default()),
        DiagnosticReport::decl(&ts_rs::Config::default()),
        CommandError::decl(&ts_rs::Config::default())
    );
    let config = ts_rs::Config::default();
    println!("export {};", velo_engine::NameValue::decl(&config));
    println!("export {};", velo_engine::RequestMethod::decl(&config));
    println!("export {};", velo_engine::RequestAuth::decl(&config));
    println!("export {};", velo_engine::MultipartPart::decl(&config));
    println!("export {};", velo_engine::RequestBody::decl(&config));
    println!("export {};", velo_engine::ResourceBudget::decl(&config));
    println!("export {};", velo_engine::RampStage::decl(&config));
    println!("export {};", velo_engine::Workload::decl(&config));
    println!("export {};", velo_engine::ThresholdMetric::decl(&config));
    println!("export {};", velo_engine::ThresholdOperator::decl(&config));
    println!("export {};", velo_engine::Threshold::decl(&config));
    println!("export {};", velo_engine::ThresholdStatus::decl(&config));
    println!("export {};", velo_engine::ThresholdResult::decl(&config));
    println!("export {};", velo_engine::RunConfig::decl(&config));
    println!("export {};", velo_engine::RunError::decl(&config));
    println!("export {};", velo_engine::RunConfirmation::decl(&config));
    println!("export {};", velo_engine::RunState::decl(&config));
    println!("export {};", velo_engine::Counters::decl(&config));
    println!("export {};", velo_engine::LatencySnapshot::decl(&config));
    println!("export {};", velo_engine::MetricsSnapshot::decl(&config));
    println!("export {};", velo_engine::ArrivalSnapshot::decl(&config));
    println!("export {};", velo_engine::RunSnapshot::decl(&config));
    println!("export {};", velo_engine::RunUpdate::decl(&config));
}
