mod args;
mod progress;
mod render;
mod ui_server;

use anyhow::Context;
use clap::Parser;
use diskgoblin_core::{
    report_to_json_writer, scan_directory_with_progress, CancelToken, FatalScanError, UnitSystem,
};
use std::io::{self, Write};
use std::process::ExitCode;

fn run(args: args::Args) -> anyhow::Result<()> {
    let cancel = CancelToken::new();
    let signal_cancel = cancel.clone();
    ctrlc::set_handler(move || signal_cancel.cancel())
        .context("failed to install SIGINT handler")?;
    if args.ui {
        return ui_server::run_ui_server(&args, cancel);
    }
    let units = if args.si {
        UnitSystem::SiDecimal
    } else {
        UnitSystem::IecBinary
    };
    let report = {
        let progress = progress::StderrProgressSink::new(args.quiet, units);
        scan_directory_with_progress(&args.scan_options(), &cancel, &progress)
    }?;
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    if args.json {
        report_to_json_writer(&report, &mut stdout)?;
        writeln!(stdout)?;
    } else {
        render::render_human_report(
            &report,
            &render::RenderOptions {
                units,
                show_errors: args.show_errors,
            },
            &mut stdout,
        )?;
    }
    stdout.flush()?;
    Ok(())
}

fn main() -> ExitCode {
    match run(args::Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("diskgoblin: {error:#}");
            if matches!(
                error.downcast_ref::<FatalScanError>(),
                Some(FatalScanError::Cancelled)
            ) {
                ExitCode::from(130)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
