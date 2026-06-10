//! Topic-key families (Q10): the keyword heuristics behind
//! `seele_suggest_topic_key`, finally configurable per consumer as
//! CLAUDE.md promised since genesis.
//!
//! Resolution order (first file that parses wins, GRAIL
//! custom-paths→builtin style):
//!
//! 1. `$SEELE_TOPIC_FAMILIES` (explicit file path — MCP servers get
//!    spawned from arbitrary CWDs, an env override is the reliable hook)
//! 2. `./.seele/topic-families.toml` (project)
//! 3. `~/.seele/topic-families.toml` (user)
//! 4. The 7 ENGRAM-inherited builtin families
//!
//! Invalid TOML or invalid families NEVER panic: the loader records a
//! warning string and falls through to the next level. The caller (the
//! service boot) decides how to log them — this module stays dependency-
//! free per seele-core's charter.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One topic family: a name (`bug`, `decision`, …) plus the lowercase
/// keywords whose presence in `title + content` votes for it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopicFamily {
    pub name: String,
    pub keywords: Vec<String>,
}

/// Where the active family set came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamilySource {
    Builtin,
    User,
    Project,
    Env,
}

impl FamilySource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::User => "user",
            Self::Project => "project",
            Self::Env => "env",
        }
    }
}

/// The resolved family set plus its provenance.
#[derive(Debug, Clone)]
pub struct FamilySet {
    pub families: Vec<TopicFamily>,
    pub source: FamilySource,
}

#[derive(Debug, Deserialize)]
struct FamiliesFile {
    #[serde(default, rename = "family")]
    families: Vec<TopicFamily>,
}

/// The 7 ENGRAM-inherited builtin families (valid for coding agents).
pub fn builtin() -> Vec<TopicFamily> {
    fn fam(name: &str, kws: &[&str]) -> TopicFamily {
        TopicFamily {
            name: name.to_string(),
            keywords: kws.iter().map(|s| s.to_string()).collect(),
        }
    }
    vec![
        fam(
            "architecture",
            &["architecture", "design", "system", "diagram", "boundary"],
        ),
        fam(
            "bug",
            &["bug", "fix", "regression", "broken", "fails", "crash"],
        ),
        fam(
            "decision",
            &["decided", "decision", "we chose", "we picked", "adr"],
        ),
        fam("pattern", &["pattern", "convention", "idiom", "approach"]),
        fam("config", &["config", "setting", "flag", "env var"]),
        fam(
            "discovery",
            &["found", "discovered", "noticed", "turns out"],
        ),
        fam("learning", &["learning", "lesson", "insight", "takeaway"]),
    ]
}

/// Parse and validate a `topic-families.toml` document. Validation is
/// minimal by design (the matching stays naive substring voting): every
/// family needs a non-empty name and at least one non-empty keyword.
pub fn parse_families_toml(raw: &str) -> Result<Vec<TopicFamily>, String> {
    let parsed: FamiliesFile = toml::from_str(raw).map_err(|e| format!("invalid TOML: {e}"))?;
    if parsed.families.is_empty() {
        return Err("no [[family]] entries".to_string());
    }
    for f in &parsed.families {
        if f.name.trim().is_empty() {
            return Err("family with empty name".to_string());
        }
        if f.keywords.iter().all(|k| k.trim().is_empty()) || f.keywords.is_empty() {
            return Err(format!("family '{}' has no usable keywords", f.name));
        }
    }
    Ok(parsed.families)
}

/// Try one candidate file. `Ok(None)` = file absent (keep falling
/// through silently); `Err` = file present but unusable (warn + fall
/// through); `Ok(Some(..))` = winner.
fn try_load(path: &Path) -> Result<Option<Vec<TopicFamily>>, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    parse_families_toml(&raw)
        .map(Some)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Resolve the active family set. Returns the set plus any warnings
/// produced by levels that existed but failed to parse — the caller logs
/// them (this crate stays tracing-free).
pub fn load_default() -> (FamilySet, Vec<String>) {
    let mut warnings = Vec::new();
    let mut candidates: Vec<(PathBuf, FamilySource)> = Vec::new();
    if let Some(env_path) = std::env::var_os("SEELE_TOPIC_FAMILIES") {
        candidates.push((PathBuf::from(env_path), FamilySource::Env));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push((
            cwd.join(".seele/topic-families.toml"),
            FamilySource::Project,
        ));
    }
    if let Some(home) = home_dir() {
        candidates.push((home.join(".seele/topic-families.toml"), FamilySource::User));
    }

    for (path, source) in candidates {
        match try_load(&path) {
            Ok(Some(families)) => {
                return (FamilySet { families, source }, warnings);
            }
            Ok(None) => {}
            Err(w) => warnings.push(w),
        }
    }
    (
        FamilySet {
            families: builtin(),
            source: FamilySource::Builtin,
        },
        warnings,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_has_the_seven_engram_families() {
        let b = builtin();
        assert_eq!(b.len(), 7);
        assert!(b.iter().any(|f| f.name == "bug"));
        assert!(b.iter().all(|f| !f.keywords.is_empty()));
    }

    #[test]
    fn parse_accepts_valid_families() {
        let fams = parse_families_toml(
            r#"
            [[family]]
            name = "ritual"
            keywords = ["ceremonia", "rito"]

            [[family]]
            name = "ops"
            keywords = ["deploy"]
            "#,
        )
        .unwrap();
        assert_eq!(fams.len(), 2);
        assert_eq!(fams[0].name, "ritual");
    }

    #[test]
    fn parse_rejects_garbage_empty_and_keywordless() {
        assert!(parse_families_toml("not toml [[[").is_err());
        assert!(parse_families_toml("").is_err());
        assert!(parse_families_toml("[[family]]\nname = \"\"\nkeywords = [\"x\"]").is_err());
        assert!(parse_families_toml("[[family]]\nname = \"x\"\nkeywords = []").is_err());
    }

    #[test]
    fn try_load_distinguishes_absent_from_broken() {
        let td = tempfile::tempdir().unwrap();
        let absent = td.path().join("nope.toml");
        assert!(matches!(try_load(&absent), Ok(None)));

        let broken = td.path().join("broken.toml");
        std::fs::write(&broken, "[[family").unwrap();
        assert!(try_load(&broken).is_err());

        let good = td.path().join("good.toml");
        std::fs::write(&good, "[[family]]\nname = \"a\"\nkeywords = [\"b\"]").unwrap();
        assert_eq!(try_load(&good).unwrap().unwrap().len(), 1);
    }
}
