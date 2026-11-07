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
    println!("export {};", velo_engine::ScenarioPlan::decl(&config));
    println!("export {};", velo_engine::Scenario::decl(&config));
    println!("export {};", velo_engine::ScenarioStep::decl(&config));
    println!("export {};", velo_engine::ScenarioRequest::decl(&config));
    println!("export {};", velo_engine::StepFailure::decl(&config));
    println!("export {};", velo_engine::Extraction::decl(&config));
    println!("export {};", velo_engine::ExtractionSource::decl(&config));
    println!("export {};", velo_engine::ScenarioReport::decl(&config));
    println!("export {};", velo_engine::ScenarioTotals::decl(&config));
    println!("export {};", velo_engine::StepTotals::decl(&config));
    println!("export {};", velo_engine::PhaseOutcome::decl(&config));
    println!("export {};", velo_engine::ScenarioPhase::decl(&config));

    println!("export {};", velo_engine::Assertion::decl(&config));
    println!("export {};", velo_engine::AssertionRule::decl(&config));
    println!("export {};", velo_engine::InspectionLimits::decl(&config));
    println!("export {};", velo_engine::CheckOutcome::decl(&config));
    println!("export {};", velo_engine::AssertionTotals::decl(&config));
    println!("export {};", velo_engine::ResponseSample::decl(&config));
    println!("export {};", velo_engine::FailureGroup::decl(&config));
    println!(
        "export {};",
        velo_engine::ResponseDiagnostics::decl(&config)
    );

    println!("export {};", velo_engine::DataLifetime::decl(&config));
    println!("export {};", velo_engine::DataAllocation::decl(&config));
    println!("export {};", velo_engine::DataExhaustion::decl(&config));
    println!("export {};", velo_engine::DataDefinition::decl(&config));
    println!("export {};", velo_engine::DataSource::decl(&config));
    println!("export {};", velo_engine::DataField::decl(&config));
    println!("export {};", velo_engine::DataGenerator::decl(&config));
    println!("export {};", velo_engine::DataRequest::decl(&config));
    println!("export {};", velo_engine::PreparedData::decl(&config));
    println!("export {};", velo_engine::DataSummary::decl(&config));

    println!("export {};", velo_contracts::HistoryEntry::decl(&config));
    println!(
        "export {};",
        velo_contracts::SavedConfiguration::decl(&config)
    );
    println!("export {};", velo_engine::TestDefinition::decl(&config));
    println!("export {};", velo_engine::ImportFormat::decl(&config));
    println!("export {};", velo_engine::ImportedRequest::decl(&config));
    println!("export {};", velo_engine::PreviewResponse::decl(&config));
    println!("export {};", velo_engine::NameValue::decl(&config));
    println!("export {};", velo_engine::RequestMethod::decl(&config));
    println!("export {};", velo_engine::RequestAuth::decl(&config));
    println!("export {};", velo_engine::HttpProtocol::decl(&config));
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
