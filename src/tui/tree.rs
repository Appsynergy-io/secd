//! One row per credential, whatever its storage shape.
//!
//! The vault is a flat map of names. A human does not think in names: a
//! Cloudflare credential is one thing whether it was stored as a single JSON
//! entry or as six siblings under a shared parent. `policy::bundle_shapes`
//! already computes that fusion for `secd run`; this turns it into a list.
//!
//! The list stays flat. Sorted full paths already cluster what belongs
//! together, and a directory level would put every value one keystroke further
//! away to say something the sort already says. The one thing `Enter` opens is
//! a bundle, onto the fields it stands for.

use std::collections::BTreeMap;

use crate::policy::BundleShape;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    /// The full path. The list dims the part shared with the row above, so a
    /// group reads as a group without costing a keystroke.
    pub label: String,
    /// The provider, when this row is a bundle.
    pub provider: Option<String>,
    /// The entries this row stands for: one for a plain value or a JSON
    /// bundle, several for siblings fused under a shared parent.
    pub members: Vec<String>,
}

impl Row {
    /// A row worth opening. A single entry has nothing below it.
    pub fn descends(&self) -> bool {
        self.members.len() > 1
    }

    /// The one entry this row is, when it is exactly one.
    pub fn only(&self) -> Option<&str> {
        match self.members.as_slice() {
            [one] => Some(one.as_str()),
            _ => None,
        }
    }

    /// How many leading characters this row shares with the one above, cut at
    /// a `/` so a group's shared prefix is dimmed and its leaf is not.
    pub fn shared(&self, prev: Option<&Row>) -> usize {
        let Some(prev) = prev else {
            return 0;
        };
        let n = self
            .label
            .bytes()
            .zip(prev.label.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        self.label[..n].rfind('/').map_or(0, |i| i + 1)
    }
}

/// Every entry, with the credential it belongs to. An entry no bundle claims
/// is its own credential of one member.
pub struct Index {
    /// entry name -> the credential's prefix.
    owner: BTreeMap<String, String>,
    /// prefix -> the credential.
    creds: BTreeMap<String, Cred>,
}

#[derive(Clone)]
struct Cred {
    provider: Option<String>,
    members: Vec<String>,
}

impl Index {
    pub fn build(names: &[String], shapes: &[BundleShape]) -> Self {
        let mut owner = BTreeMap::new();
        let mut creds: BTreeMap<String, Cred> = BTreeMap::new();
        for s in shapes {
            // A shape naming an entry the register does not hold would put a
            // row on screen that nothing can open.
            let members: Vec<String> = s
                .members
                .iter()
                .filter(|m| names.iter().any(|n| n == *m))
                .cloned()
                .collect();
            if members.is_empty() {
                continue;
            }
            for m in &members {
                owner.insert(m.clone(), s.name.clone());
            }
            creds.insert(
                s.name.clone(),
                Cred {
                    provider: s.provider.clone(),
                    members,
                },
            );
        }
        for n in names {
            if owner.contains_key(n) {
                continue;
            }
            owner.insert(n.clone(), n.clone());
            creds.insert(
                n.clone(),
                Cred {
                    provider: None,
                    members: vec![n.clone()],
                },
            );
        }
        Self { owner, creds }
    }

    /// The credential an entry belongs to, so a save can put the selection
    /// back on the row the human was looking at.
    pub fn owner_of(&self, name: &str) -> Option<&str> {
        self.owner.get(name).map(String::as_str)
    }

    pub fn is_bundle(&self, prefix: &str) -> bool {
        self.creds.get(prefix).is_some_and(|c| c.members.len() > 1)
    }

    /// The rows to draw. `open` is the bundle being looked into, empty for the
    /// register itself. `filter` matches anywhere in the path or the provider,
    /// so `github` finds it wherever it is filed.
    pub fn rows(&self, open: &str, filter: &str) -> Vec<Row> {
        if let Some(cred) = self.creds.get(open) {
            if cred.members.len() > 1 {
                return cred
                    .members
                    .iter()
                    .map(|m| Row {
                        label: m.clone(),
                        provider: None,
                        members: vec![m.clone()],
                    })
                    .collect();
            }
        }
        let want = filter.to_ascii_lowercase();
        self.creds
            .iter()
            .filter(|(prefix, cred)| {
                want.is_empty()
                    || prefix.to_ascii_lowercase().contains(&want)
                    || cred
                        .provider
                        .as_deref()
                        .is_some_and(|p| p.to_ascii_lowercase().contains(&want))
            })
            .map(|(prefix, cred)| Row {
                label: prefix.clone(),
                provider: cred.provider.clone(),
                members: cred.members.clone(),
            })
            .collect()
    }

    /// Credentials in the register, ignoring the filter, for the list title.
    pub fn total(&self) -> usize {
        self.creds.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(name: &str, provider: &str, members: &[&str]) -> BundleShape {
        BundleShape {
            name: name.to_string(),
            provider: Some(provider.to_string()),
            members: members.iter().map(|m| (*m).to_string()).collect(),
        }
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    fn labels(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|r| r.label.as_str()).collect()
    }

    #[test]
    fn siblings_are_one_row() {
        let n = names(&["prod/github/token", "prod/github/user", "prod/note"]);
        let s = [shape(
            "prod/github",
            "github",
            &["prod/github/token", "prod/github/user"],
        )];
        let ix = Index::build(&n, &s);
        let rows = ix.rows("", "");
        assert_eq!(
            labels(&rows),
            ["prod/github", "prod/note"],
            "the pair is one row, not two"
        );
        assert_eq!(rows[0].members.len(), 2, "and it stands for both entries");
        assert_eq!(rows[0].provider.as_deref(), Some("github"));
        assert!(rows[0].descends(), "a two-entry bundle opens");
        assert!(!rows[1].descends(), "a lone value does not");
        assert_eq!(ix.total(), 2);
    }

    #[test]
    fn opening_a_bundle_shows_its_fields() {
        let n = names(&["prod/github/token", "prod/github/user"]);
        let s = [shape(
            "prod/github",
            "github",
            &["prod/github/token", "prod/github/user"],
        )];
        let ix = Index::build(&n, &s);
        assert!(ix.is_bundle("prod/github"));
        let inside = ix.rows("prod/github", "");
        assert_eq!(labels(&inside), ["prod/github/token", "prod/github/user"]);
        assert!(inside.iter().all(|r| !r.descends()), "and stops there");
    }

    #[test]
    fn a_stray_name_is_its_own_row() {
        let n = names(&["kv/one"]);
        let ix = Index::build(&n, &[]);
        assert_eq!(ix.owner_of("kv/one"), Some("kv/one"));
        assert!(!ix.is_bundle("kv/one"));
        let rows = ix.rows("", "");
        assert_eq!(rows[0].members, ["kv/one"]);
        assert_eq!(rows[0].provider, None);
    }

    #[test]
    fn filter_matches_the_path_or_the_provider() {
        let n = names(&["a/gh/token", "a/gh/user", "b/slack"]);
        let s = [shape("a/gh", "github", &["a/gh/token", "a/gh/user"])];
        let ix = Index::build(&n, &s);
        assert_eq!(labels(&ix.rows("", "slack")), ["b/slack"]);
        assert_eq!(
            labels(&ix.rows("", "github")),
            ["a/gh"],
            "the provider is searchable even when the path never says it"
        );
        assert!(ix.rows("", "nothing").is_empty());
        assert_eq!(ix.total(), 2, "the total ignores the filter");
    }

    #[test]
    fn a_shape_naming_a_missing_entry_is_dropped() {
        let n = names(&["p/x/token"]);
        let s = [shape("p/x", "github", &["p/x/token", "p/x/gone"])];
        let ix = Index::build(&n, &s);
        assert_eq!(ix.rows("", "")[0].members, ["p/x/token"]);
    }

    #[test]
    fn shared_prefix_stops_at_a_slash() {
        let rows = [
            Row {
                label: "prod/github".into(),
                provider: None,
                members: vec![],
            },
            Row {
                label: "prod/gitlab".into(),
                provider: None,
                members: vec![],
            },
            Row {
                label: "kv/x".into(),
                provider: None,
                members: vec![],
            },
        ];
        assert_eq!(rows[0].shared(None), 0, "the first row shares nothing");
        assert_eq!(
            rows[1].shared(Some(&rows[0])),
            5,
            "`prod/` is shared; `git` is not, though the bytes match"
        );
        assert_eq!(rows[2].shared(Some(&rows[1])), 0);
    }
}
