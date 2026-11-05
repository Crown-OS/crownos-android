//! `crown-android-bench`: measures the CrownOS Android runtime against the
//! plan's budgets and fails on misses and regressions.

mod budget;
mod cli;
mod findings;
mod host;
mod measure;
mod parse;
mod report;
mod run;
mod unit;
mod util;
mod verdict;
mod wayland;

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

use crate::budget::Budgets;
use crate::findings::Findings;
use crate::host::{Host, Privilege};
use crate::report::RunReport;
use crate::util::timestamp::UtcTime;
use crate::verdict::{Verdict, judge};

fn main() -> Result<ExitCode> {
    let args = cli::Args::parse();
    let budgets = Budgets::load(args.budgets.as_deref())?;
    budgets.check_covers(&measure::METRICS)?;
    let baseline = args.baseline.as_deref().map(report::load).transpose()?;
    let baseline_metrics = baseline.as_ref().map(|baseline| &baseline.metrics);

    if let Some(result) = &args.compare {
        let findings = report::load(result)?;
        let verdicts = judge(
            &budgets,
            &findings.metrics,
            &findings.skipped,
            baseline_metrics,
        );
        return conclude(&verdicts, &findings);
    }

    let directory = match &args.out {
        Some(directory) => directory.clone(),
        None => report::default_directory()?,
    };
    std::fs::create_dir_all(&directory)?;
    let started = UtcTime::now();
    let host = Host::system();
    let privilege = Privilege::current();
    let findings = run::run(&args, &host, privilege, &directory);
    let verdicts = judge(
        &budgets,
        &findings.metrics,
        &findings.skipped,
        baseline_metrics,
    );
    let result = RunReport {
        format: report::FORMAT,
        started: started.iso8601(),
        kernel: host.kernel_release(),
        privilege,
        apps: &args.apps,
        baseline: args.baseline.as_deref(),
        verdicts: &verdicts,
        findings: &findings,
    };
    let path = report::write(&directory, &started.compact(), &result)?;
    println!("results: {}", path.display());
    conclude(&verdicts, &findings)
}

fn conclude(verdicts: &[Verdict<'_>], findings: &Findings) -> Result<ExitCode> {
    report::print_summary(verdicts, &findings.failures, &mut std::io::stdout().lock())?;
    let failed =
        !findings.failures.is_empty() || verdicts.iter().any(|verdict| verdict.status.fails());
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
