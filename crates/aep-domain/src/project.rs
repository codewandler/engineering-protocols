//! What a project says about itself.
//!
//! A project adopting AEP keeps one small file — `.engineering/project.yaml` — that names the
//! protocol it runs under, the profile it uses, and where the protocol documents live. Everything
//! else is discovered from it.
//!
//! ```yaml
//! version: aep.project/1
//! protocol: adp/1
//! profile: development.standard
//! protocols: ../engineering-protocols   # where the protocol tree is
//! artifacts: artifacts.yaml
//! task: task.yaml
//! ```
//!
//! # Why this file is deliberately thin
//!
//! It points; it does not duplicate. A project that restated its principles here would have two
//! copies of its rules and no way to tell which one was in force. The one thing it *may* add is
//! documents of its own, under `.engineering/principles/` and `.engineering/profiles/`, because no
//! organisation's rules are entirely somebody else's — and those are documents in the same format,
//! validated the same way, not a second mechanism.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::error::{ValidationCode, ValidationError, ValidationErrors};
use crate::version::{ProfileVersionedRef, ProtocolRef};

/// The directory a project keeps its machine-readable metadata in.
pub const PROJECT_DIRECTORY: &str = ".engineering";
/// The file naming the protocol, the profile and where the documents are.
pub const PROJECT_FILE: &str = "project.yaml";
/// The format version this build reads.
pub const PROJECT_VERSION: &str = "aep.project/1";

/// Where a project keeps each thing.
///
/// Paths are relative to the `.engineering` directory, so a project can be moved or vendored without
/// editing them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectPaths {
    /// The protocol document tree.
    pub protocols: PathBuf,
    /// The artifact manifest.
    pub artifacts: PathBuf,
    /// The task being worked on.
    pub task: PathBuf,
    /// Where an execution's state is kept between runs.
    pub state: PathBuf,
    /// Project-local principles, merged over the protocol tree's.
    pub principles: PathBuf,
    /// Project-local profiles.
    pub profiles: PathBuf,
}

impl Default for ProjectPaths {
    fn default() -> Self {
        Self {
            protocols: PathBuf::from(".."),
            artifacts: PathBuf::from("artifacts.yaml"),
            task: PathBuf::from("task.yaml"),
            state: PathBuf::from("state.yaml"),
            principles: PathBuf::from("principles"),
            profiles: PathBuf::from("profiles"),
        }
    }
}

impl ProjectPaths {
    /// Resolves every path against the `.engineering` directory.
    #[must_use]
    pub fn resolved(&self, engineering: &Path) -> Self {
        Self {
            protocols: engineering.join(&self.protocols),
            artifacts: engineering.join(&self.artifacts),
            task: engineering.join(&self.task),
            state: engineering.join(&self.state),
            principles: engineering.join(&self.principles),
            profiles: engineering.join(&self.profiles),
        }
    }
}

/// What a project says about itself.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectConfig {
    /// The protocol version it runs under.
    pub protocol: ProtocolRef,
    /// The profile it uses.
    pub profile: ProfileVersionedRef,
    /// A one-line description, for a report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Where each thing lives.
    pub paths: ProjectPaths,
}

impl fmt::Display for ProjectConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} under {}", self.profile, self.protocol)
    }
}

/// A project configuration document, as parsed.
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawProjectConfig {
    /// The format version.
    #[serde(default = "default_version")]
    pub version: String,
    /// The protocol version this project runs under.
    pub protocol: ProtocolRef,
    /// The profile it uses.
    pub profile: ProfileVersionedRef,
    /// A one-line description.
    #[serde(default)]
    pub summary: Option<String>,
    /// Where the protocol document tree is, relative to `.engineering`.
    #[serde(default)]
    pub protocols: Option<PathBuf>,
    /// Where the artifact manifest is.
    #[serde(default)]
    pub artifacts: Option<PathBuf>,
    /// Where the task document is.
    #[serde(default)]
    pub task: Option<PathBuf>,
    /// Where execution state is kept.
    #[serde(default)]
    pub state: Option<PathBuf>,
    /// Where project-local principles are.
    #[serde(default)]
    pub principles: Option<PathBuf>,
    /// Where project-local profiles are.
    #[serde(default)]
    pub profiles: Option<PathBuf>,
}

/// Serde default for the format version.
fn default_version() -> String {
    PROJECT_VERSION.to_owned()
}

impl TryFrom<RawProjectConfig> for ProjectConfig {
    type Error = ValidationErrors;

    fn try_from(raw: RawProjectConfig) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();

        if raw.version != PROJECT_VERSION {
            errors.push(
                ValidationError::new(
                    ValidationCode::UnsupportedProtocolVersion,
                    "project.version",
                    format!(
                        "this build reads `{PROJECT_VERSION}`, not `{}`",
                        raw.version
                    ),
                )
                .with_hint("upgrade the tooling rather than reinterpreting the document"),
            );
        }

        let defaults = ProjectPaths::default();
        let paths = ProjectPaths {
            protocols: raw.protocols.unwrap_or(defaults.protocols),
            artifacts: raw.artifacts.unwrap_or(defaults.artifacts),
            task: raw.task.unwrap_or(defaults.task),
            state: raw.state.unwrap_or(defaults.state),
            principles: raw.principles.unwrap_or(defaults.principles),
            profiles: raw.profiles.unwrap_or(defaults.profiles),
        };

        // Only paths *inside* the project must be relative. `protocols` may point anywhere: the
        // protocol tree is often a sibling checkout or a vendored copy under a machine-specific
        // path, and forbidding that would force a symlink for no gain.
        for (label, path) in [
            ("artifacts", &paths.artifacts),
            ("task", &paths.task),
            ("state", &paths.state),
            ("principles", &paths.principles),
            ("profiles", &paths.profiles),
        ] {
            if path.is_absolute() {
                errors.push(
                    ValidationError::new(
                        ValidationCode::TypeMismatch,
                        format!("project.{label}"),
                        format!("`{}` is absolute", path.display()),
                    )
                    .with_hint(
                        "paths inside the project are relative to `.engineering`, so the repository \
                         can be cloned anywhere without editing them; only `protocols` may be \
                         absolute, because the protocol tree often lives outside the project",
                    ),
                );
            }
        }

        let config = Self {
            protocol: raw.protocol,
            profile: raw.profile,
            summary: raw.summary,
            paths,
        };
        errors.into_result(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(yaml: &str) -> Result<ProjectConfig, ValidationErrors> {
        let raw: RawProjectConfig = serde_yaml::from_str(yaml).expect("document parses");
        ProjectConfig::try_from(raw)
    }

    #[test]
    fn a_minimal_project_file_names_only_what_it_must() {
        let parsed = config(
            r"
protocol: adp/1
profile: development.standard
",
        )
        .expect("validates");

        assert_eq!(parsed.protocol.to_string(), "adp/1");
        assert_eq!(parsed.profile.to_string(), "development.standard");
        assert_eq!(parsed.paths.artifacts, PathBuf::from("artifacts.yaml"));
        assert_eq!(
            parsed.paths.protocols,
            PathBuf::from(".."),
            "a project vendoring the protocol tree beside itself is the common case"
        );
    }

    #[test]
    fn paths_resolve_against_the_engineering_directory() {
        let parsed = config(
            r"
protocol: adp/1
profile: development.standard
protocols: ../../protocols
artifacts: graph.yaml
",
        )
        .expect("validates");

        let resolved = parsed
            .paths
            .resolved(Path::new("/work/payments/.engineering"));
        assert_eq!(
            resolved.artifacts,
            PathBuf::from("/work/payments/.engineering/graph.yaml")
        );
        assert_eq!(
            resolved.protocols,
            PathBuf::from("/work/payments/.engineering/../../protocols")
        );
    }

    #[test]
    fn the_protocol_tree_may_live_outside_the_project() {
        let parsed = config(
            r"
protocol: adp/1
profile: development.standard
protocols: /opt/engineering-protocols
",
        )
        .expect("an absolute protocol tree is allowed");
        assert_eq!(
            parsed.paths.protocols,
            PathBuf::from("/opt/engineering-protocols")
        );
    }

    #[test]
    fn an_absolute_path_is_refused() {
        let errors = config(
            r"
protocol: adp/1
profile: development.standard
artifacts: /etc/engineering/artifacts.yaml
",
        )
        .expect_err("absolute path");
        assert!(
            errors.to_string().contains("cloned anywhere"),
            "the refusal must say why relative paths matter: {errors}"
        );
    }

    #[test]
    fn an_unknown_format_version_is_refused_rather_than_guessed() {
        let errors = config(
            r"
version: aep.project/9
protocol: adp/1
profile: development.standard
",
        )
        .expect_err("unknown version");
        assert!(errors.contains(ValidationCode::UnsupportedProtocolVersion));
    }

    #[test]
    fn an_unknown_key_is_refused_rather_than_ignored() {
        let raw: Result<RawProjectConfig, _> = serde_yaml::from_str(
            r"
protocol: adp/1
profile: development.standard
artefacts: graph.yaml
",
        );
        assert!(
            raw.is_err(),
            "a misspelled key that is silently ignored is a project pointing at nothing"
        );
    }
}
