//! The `gatepass` system, realized, on the wire.
//!
//! Forty lines, and the arrow points the way it always points here: this binary links the
//! hand-written realization into the generated surface, and the generated surface knows nothing
//! about it. `gatepass_server::pass_service::serve` binds, writes the startup record and answers
//! the routes the committed `OpenAPI` document declares; everything it answers *with* comes
//! through the port from [`gatepass_realization::linker`].
//!
//! # The port comes from the environment, not from an argument
//!
//! `PORT` unset or `0` binds an ephemeral port, which is what makes the gate's demonstration
//! deterministic: two of these run side by side without agreeing about a number in advance, and
//! each says in its startup record which port it took. There is no argument parsing here at all —
//! a synthesised surface takes no options, so there is nothing to parse.

use std::process::ExitCode;

fn main() -> ExitCode {
    let port = std::env::var("PORT").unwrap_or_else(|_| "0".to_owned());
    let address = format!("127.0.0.1:{port}");
    let mut assembled = gatepass_realization::linker::honest();
    match gatepass_server::pass_service::serve(&mut assembled.system, &address) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"log\":\"ess/1\",\"event\":\"system.stopped\",\"reason\":\"{error}\"}}");
            ExitCode::FAILURE
        }
    }
}
