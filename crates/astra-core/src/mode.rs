//! Compile-time capability modes.
//!
//! Mode compatibility is a containment relation rather than a single global
//! rank: `web` and `system` expose different capabilities, and neither contains
//! the other. The relation is used by the checker for `@requires` annotations,
//! capability tags, and nested `@mode:` scopes.

use serde::{Deserialize, Serialize};

use crate::span::Span;

/// A Neo Astra capability mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Core,
    App,
    Web,
    System,
}

impl Mode {
    /// The lowercase name used in manifests and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::App => "app",
            Self::Web => "web",
            Self::System => "system",
        }
    }

    /// Parses a mode name.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "core" => Some(Self::Core),
            "app" => Some(Self::App),
            "web" => Some(Self::Web),
            "system" => Some(Self::System),
            _ => None,
        }
    }

    /// Returns whether this mode grants everything required by `required`.
    #[must_use]
    pub const fn allows(self, required: Self) -> bool {
        matches!(
            (self, required),
            (Self::Core, Self::Core)
                | (Self::App, Self::Core | Self::App)
                | (Self::Web, Self::Core | Self::App | Self::Web)
                | (Self::System, Self::Core | Self::App | Self::System)
        )
    }

    /// The minimum mode implied by a capability tag.
    ///
    /// Tags are intentionally a small, documented vocabulary in v0.1. Unknown
    /// tags are treated as user-defined and do not grant or require a built-in
    /// capability.
    #[must_use]
    pub fn requirement_for_capability(tag: &str) -> Option<Self> {
        match tag {
            "console" | "args" | "time" | "env" => Some(Self::App),
            "dom" | "fetch" | "worker" | "storage" | "websocket" => Some(Self::Web),
            "fs" | "net" | "process" | "ffi" | "thread" | "ptr" | "memory" | "os" => {
                Some(Self::System)
            }
            _ => None,
        }
    }
}

/// A source-level mode requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModeRequirement {
    pub mode: Mode,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::Mode;

    #[test]
    fn mode_names_match_the_manifest_contract() {
        assert_eq!(Mode::Core.as_str(), "core");
        assert_eq!(Mode::App.as_str(), "app");
        assert_eq!(Mode::Web.as_str(), "web");
        assert_eq!(Mode::System.as_str(), "system");
    }

    #[test]
    fn web_and_system_are_incomparable() {
        assert!(Mode::Web.allows(Mode::Web));
        assert!(!Mode::Web.allows(Mode::System));
        assert!(Mode::System.allows(Mode::System));
        assert!(!Mode::System.allows(Mode::Web));
    }

    #[test]
    fn narrower_modes_are_always_allowed() {
        assert!(Mode::System.allows(Mode::Core));
        assert!(Mode::Web.allows(Mode::App));
        assert!(Mode::App.allows(Mode::Core));
        assert!(!Mode::Core.allows(Mode::App));
    }
}
