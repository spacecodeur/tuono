use std::sync::Arc;
use watchexec_supervisor::command::{Command, Program};

use miette::{IntoDiagnostic, Result};
use watchexec::Watchexec;
use watchexec_signals::Signal;
use watchexec_supervisor::job::{start_job, Job};

use crate::source_builder::{bundle_axum_source, Mode};

fn watch_react_src() -> Job {
    start_job(Arc::new(Command {
        program: Program::Exec {
            prog: "node_modules/.bin/tuono-dev-watch".into(),
            args: vec![],
        },
        options: Default::default(),
    }))
    .0
}

fn build_rust_src() -> Job {
    start_job(Arc::new(Command {
        program: Program::Exec {
            prog: "cargo".into(),
            args: vec!["run".to_string(), "-q".to_string()],
        },
        options: Default::default(),
    }))
    .0
}

fn build_react_ssr_src() -> Job {
    start_job(Arc::new(Command {
        program: Program::Exec {
            prog: "node_modules/.bin/tuono-dev-ssr".into(),
            args: vec![],
        },
        options: Default::default(),
    }))
    .0
}

#[tokio::main]
pub async fn watch() -> Result<()> {
    println!("Starting development environment...");
    watch_react_src().start().await;

    let run_server = build_rust_src();

    let build_ssr_bundle = build_react_ssr_src();

    build_ssr_bundle.start().await;

    run_server.start().await;

    build_ssr_bundle.to_wait().await;

    let wx = Watchexec::new(move |mut action| {
        let mut should_reload_ssr_bundle = false;
        let mut should_reload_rust_server = false;

        for event in action.events.iter() {
            for path in event.paths() {
                if path.0.to_string_lossy().ends_with(".rs") {
                    should_reload_rust_server = true
                }

                // Either tsx and jsx
                if path.0.to_string_lossy().ends_with("sx") {
                    should_reload_ssr_bundle = true
                }
            }
        }

        if should_reload_rust_server {
            println!("Reloading...");
            run_server.stop();
            bundle_axum_source(Mode::Dev).expect("Failed to bunlde rust source");
            run_server.start();
        }

        if should_reload_ssr_bundle {
            build_ssr_bundle.stop();
            build_ssr_bundle.start();
        }

        // if Ctrl-C is received, quit
        if action.signals().any(|sig| sig == Signal::Interrupt) {
            action.quit();
        }

        action
    })?;

    // watch the current directory
    wx.config.pathset(["./src"]);

    let _ = wx.main().await.into_diagnostic()?;
    Ok(())
}
