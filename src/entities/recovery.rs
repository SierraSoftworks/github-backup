use std::fmt::Display;
use std::str::FromStr;

/// Controls how the git backup engine responds when an existing local copy of
/// a repository cannot be updated due to a local problem, such as stale lock
/// files left behind by an interrupted run, or a corrupted repository.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RecoveryMode {
    /// Never attempt automatic recovery; report the error and leave the local
    /// repository untouched.
    Disabled,

    /// Attempt only non-destructive recovery steps (such as removing stale git
    /// lock files) before retrying the backup. This is the default.
    #[default]
    NonDestructive,

    /// In addition to the non-destructive recovery steps, allow the engine to
    /// clone a fresh copy of the repository into a temporary location and
    /// replace the local copy with it if the clone succeeds.
    Destructive,
}

impl FromStr for RecoveryMode {
    type Err = human_errors::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "none" | "disabled" | "off" => Ok(RecoveryMode::Disabled),
            "non-destructive" | "nondestructive" => Ok(RecoveryMode::NonDestructive),
            "destructive" => Ok(RecoveryMode::Destructive),
            other => Err(human_errors::user(
                format!("The recovery mode '{other}' is not recognized."),
                &[
                    "Use one of 'none', 'non-destructive', or 'destructive' as the 'recovery' property in your backup policy.",
                ],
            )),
        }
    }
}

impl Display for RecoveryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecoveryMode::Disabled => write!(f, "none"),
            RecoveryMode::NonDestructive => write!(f, "non-destructive"),
            RecoveryMode::Destructive => write!(f, "destructive"),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("none", RecoveryMode::Disabled)]
    #[case("disabled", RecoveryMode::Disabled)]
    #[case("off", RecoveryMode::Disabled)]
    #[case("non-destructive", RecoveryMode::NonDestructive)]
    #[case("nondestructive", RecoveryMode::NonDestructive)]
    #[case("destructive", RecoveryMode::Destructive)]
    #[case("Destructive", RecoveryMode::Destructive)]
    #[case(" destructive ", RecoveryMode::Destructive)]
    fn parse(#[case] input: &str, #[case] expected: RecoveryMode) {
        assert_eq!(input.parse::<RecoveryMode>().unwrap(), expected);
    }

    #[rstest]
    #[case("")]
    #[case("bogus")]
    #[case("delete-everything")]
    fn parse_invalid(#[case] input: &str) {
        input
            .parse::<RecoveryMode>()
            .expect_err("parsing should fail for unrecognized recovery modes");
    }

    #[test]
    fn default_is_non_destructive() {
        assert_eq!(RecoveryMode::default(), RecoveryMode::NonDestructive);
    }

    #[rstest]
    #[case(RecoveryMode::Disabled, "none")]
    #[case(RecoveryMode::NonDestructive, "non-destructive")]
    #[case(RecoveryMode::Destructive, "destructive")]
    fn display_round_trips(#[case] mode: RecoveryMode, #[case] display: &str) {
        assert_eq!(format!("{mode}"), display);
        assert_eq!(display.parse::<RecoveryMode>().unwrap(), mode);
    }
}
