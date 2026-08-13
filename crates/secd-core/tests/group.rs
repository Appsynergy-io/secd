#![allow(non_snake_case)]

use secd_core::{group_names, NameGroup};

fn group<'a>(groups: &'a [NameGroup], name: &str) -> &'a NameGroup {
    groups
        .iter()
        .find(|g| g.name == name)
        .unwrap_or_else(|| panic!("missing group {name}, have {groups:?}"))
}

#[test]
fn T_GROUP_PREFIX() {
    let names = ["lab/only"];
    let groups = group_names(&names, &["lab"]);
    assert_eq!(groups.len(), 1);
    let lab = group(&groups, "lab");
    assert_eq!(lab.members, ["lab/only"]);

    let names = ["lab/alpha", "lab/beta", "solo/x"];
    let groups = group_names(&names, &["lab"]);
    assert!(
        groups.iter().all(|g| g.name != "solo"),
        "unregistered singleton directory is not a set: {groups:?}"
    );
    let lab = group(&groups, "lab");
    assert_eq!(lab.members, ["lab/alpha", "lab/beta"]);
}

#[test]
fn T_GROUP_DIR() {
    let names = ["proj/alpha", "proj/beta", "solo/only"];
    let groups = group_names(&names, &[]);
    assert_eq!(groups.len(), 1, "only the ≥2-member directory is a set");
    let proj = group(&groups, "proj");
    assert_eq!(proj.members, ["proj/alpha", "proj/beta"]);
    assert!(
        groups.iter().all(|g| g.name != "solo"),
        "directory with one member is not a set: {groups:?}"
    );
}

#[test]
fn T_GROUP_STRAY() {
    let names = ["k1/a", "k1/b", "init-k1"];
    let groups = group_names(&names, &[]);
    let k1 = group(&groups, "k1");
    assert!(
        k1.members.iter().any(|m| m == "init-k1"),
        "init-k1 beside k1/ must join: {k1:?}"
    );
    assert!(k1.members.iter().any(|m| m == "k1/a"), "{k1:?}");
    assert!(k1.members.iter().any(|m| m == "k1/b"), "{k1:?}");
    assert_eq!(k1.members.len(), 3, "{k1:?}");
    assert!(
        groups.iter().all(|g| g.name != "init-k1"),
        "init-k1 is not its own group: {groups:?}"
    );
}
