/// One set produced by [`group_names`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameGroup {
    pub name: String,
    pub members: Vec<String>,
}

/// Three rules: a registered prefix owns children; a directory with ≥2 members is a set;
/// `init-X` beside `X/` joins that set.
pub fn group_names(names: &[&str], registered: &[&str]) -> Vec<NameGroup> {
    let mut assigned = vec![false; names.len()];
    let mut groups: Vec<NameGroup> = Vec::new();

    let mut prefixes: Vec<&str> = registered.to_vec();
    for p in crate::providers() {
        if !prefixes.contains(&p.name.as_str()) {
            prefixes.push(p.name.as_str());
        }
    }
    prefixes.sort_by_key(|p| std::cmp::Reverse(p.len()));
    prefixes.dedup();

    for prefix in prefixes {
        let mut members = Vec::new();
        for (i, name) in names.iter().enumerate() {
            if assigned[i] {
                continue;
            }
            if owned_by(name, prefix) {
                assigned[i] = true;
                members.push((*name).to_string());
            }
        }
        if !members.is_empty() {
            groups.push(NameGroup {
                name: prefix.to_string(),
                members,
            });
        }
    }

    let mut dirs: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, name) in names.iter().enumerate() {
        if assigned[i] {
            continue;
        }
        let Some((dir, _)) = name.split_once('/') else {
            continue;
        };
        if let Some((_, idxs)) = dirs.iter_mut().find(|(d, _)| d == dir) {
            idxs.push(i);
        } else {
            dirs.push((dir.to_string(), vec![i]));
        }
    }
    for (dir, idxs) in dirs {
        if idxs.len() < 2 {
            continue;
        }
        let mut members = Vec::new();
        for i in idxs {
            assigned[i] = true;
            members.push(names[i].to_string());
        }
        groups.push(NameGroup { name: dir, members });
    }

    for (i, name) in names.iter().enumerate() {
        if assigned[i] {
            continue;
        }
        let Some(x) = name.strip_prefix("init-") else {
            continue;
        };
        if x.is_empty() || x.contains('/') {
            continue;
        }
        if !beside(names, x) {
            continue;
        }
        assigned[i] = true;
        if let Some(g) = groups.iter_mut().find(|g| g.name == x) {
            g.members.push((*name).to_string());
            continue;
        }
        let mut members = vec![(*name).to_string()];
        for (j, other) in names.iter().enumerate() {
            if assigned[j] {
                continue;
            }
            if owned_by(other, x) {
                assigned[j] = true;
                members.push((*other).to_string());
            }
        }
        groups.push(NameGroup {
            name: x.to_string(),
            members,
        });
    }

    groups
}

fn owned_by(name: &str, prefix: &str) -> bool {
    name == prefix || (name.starts_with(prefix) && name.as_bytes().get(prefix.len()) == Some(&b'/'))
}

fn beside(names: &[&str], x: &str) -> bool {
    names
        .iter()
        .any(|n| n.starts_with(x) && n.as_bytes().get(x.len()) == Some(&b'/'))
}
