//! Which reader a transcript gets, decided from the file rather than from a flag.
//!
//! There are two adapters now — the Claude Code `stream-json` reader in [`crate::adapter`] and the
//! metaharness event-stream reader in [`crate::event_stream`] — and a caller should not have to
//! know which one a file needs. `protocol trace check --transcript x.jsonl` takes the same flags
//! for a recorded vendor transcript and for a driven run's event stream, because the alternative is
//! a `--format` argument that is wrong exactly when somebody is in a hurry: a mislabelled file
//! either refuses loudly or, worse, reads as an empty run.
//!
//! # The rule
//!
//! The **first non-blank line** decides. If it is JSON carrying `format: metaharness.event/1`, the
//! file is an event stream; anything else goes to the `stream-json` reader. Two properties make
//! that safe:
//!
//! * the event wire tags **every** line, so the first line is enough to recognise it and
//!   [`crate::event_stream`] still checks the rest — a file whose halves came from two wires is
//!   refused rather than half-read;
//! * `stream-json` is the **fallback**, not a second guess. A file that is neither reaches the
//!   reader whose refusals are written for a file that is not a transcript, which is what a caller
//!   who passed the wrong path needs to read.
//!
//! Nothing here sniffs content beyond that tag. A format claim is a thing a producer wrote down on
//! purpose; guessing from the shape of the first object would be a reader inventing a fact about a
//! file it was handed.

use trace_domain::code::ValidationErrors;
use trace_domain::ir::TraceIr;

use crate::adapter::read_transcript;
use crate::event_stream::{read_event_stream, EVENT_STREAM_FORMAT};

/// Which wire a transcript is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptFormat {
    /// A `metaharness.event/1` event stream — what a driven `llm` step writes.
    MetaharnessEventStream,
    /// Claude Code `stream-json` — what the recorded fixtures are, and the fallback.
    ClaudeStreamJson,
}

impl TranscriptFormat {
    /// The wire's name, as an error message or a report prints it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MetaharnessEventStream => EVENT_STREAM_FORMAT,
            Self::ClaudeStreamJson => "claude-code/stream-json",
        }
    }
}

/// Which reader these bytes need, by the rule this module documents.
#[must_use]
pub fn detect(bytes: &[u8]) -> TranscriptFormat {
    let Ok(text) = std::str::from_utf8(bytes) else {
        // Not text at all. The `stream-json` reader owns that refusal, and its message says so.
        return TranscriptFormat::ClaudeStreamJson;
    };
    let Some(first) = text.lines().find(|line| !line.trim().is_empty()) else {
        return TranscriptFormat::ClaudeStreamJson;
    };
    let tagged = serde_json::from_str::<serde_json::Value>(first)
        .ok()
        .and_then(|value| {
            value
                .get("format")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        });
    if tagged.as_deref() == Some(EVENT_STREAM_FORMAT) {
        TranscriptFormat::MetaharnessEventStream
    } else {
        TranscriptFormat::ClaudeStreamJson
    }
}

/// Reads a transcript in whichever of the two wires it is written in.
///
/// # Errors
///
/// Whatever the chosen reader refuses: `TRACE-ADAPT-001` for a file that is not a transcript and
/// `TRACE-ADAPT-002` for one with no events. The verdict a caller gets is the chosen reader's, and
/// the returned IR names which adapter produced it — [`TraceIr::adapter`] — so a report says who
/// read the run.
pub fn read_any(bytes: &[u8]) -> Result<TraceIr, ValidationErrors> {
    match detect(bytes) {
        TranscriptFormat::MetaharnessEventStream => read_event_stream(bytes),
        TranscriptFormat::ClaudeStreamJson => read_transcript(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVENT: &str = r#"{"format":"metaharness.event/1","seq":1,"run":"T-1/1","event":"session.ended","is_error":false}"#;
    const STREAM_JSON: &str = r#"{"type":"result","subtype":"success","is_error":false}"#;

    #[test]
    fn a_tagged_first_line_sends_the_file_to_the_event_stream_reader() {
        assert_eq!(
            detect(EVENT.as_bytes()),
            TranscriptFormat::MetaharnessEventStream
        );
        let ir = read_any(EVENT.as_bytes()).expect("an event stream reads");
        assert_eq!(ir.adapter.name, "metaharness/event-stream");
    }

    #[test]
    fn a_transcript_with_no_format_tag_falls_back_to_the_stream_json_reader() {
        // The recorded fixtures are this shape and must keep reading unchanged.
        assert_eq!(
            detect(STREAM_JSON.as_bytes()),
            TranscriptFormat::ClaudeStreamJson
        );
        let ir = read_any(STREAM_JSON.as_bytes()).expect("a stream-json transcript reads");
        assert_eq!(ir.adapter.name, "claude-code/stream-json");
    }

    #[test]
    fn a_blank_leading_line_does_not_hide_the_tag() {
        let padded = format!("\n\n{EVENT}\n");
        assert_eq!(
            detect(padded.as_bytes()),
            TranscriptFormat::MetaharnessEventStream
        );
    }

    #[test]
    fn a_file_that_is_neither_is_refused_by_the_reader_whose_message_says_so() {
        // Detection never refuses; it chooses. A caller who passed the wrong path gets the
        // fallback reader's refusal, which is written for exactly that mistake.
        let errors = read_any(b"# a shell script\necho hi\n").expect_err("not a transcript");
        assert!(
            errors.as_slice()[0].message.contains("line 1"),
            "{}",
            errors.as_slice()[0].message
        );
    }

    #[test]
    fn a_tag_from_another_metaharness_wire_is_not_this_one() {
        // `metaharness.command/1` is the other direction of the same seam. Sending it to the event
        // reader would produce a stream of refusals; sending it to the fallback produces the
        // honest "this is not a transcript".
        let command =
            r#"{"format":"metaharness.command/1","id":"decide-c-1","command":"tool.decide"}"#;
        assert_eq!(
            detect(command.as_bytes()),
            TranscriptFormat::ClaudeStreamJson
        );
    }
}
