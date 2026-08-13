const REPLACEMENT: &str = "[redacted]";

/// Replace each whole `values` occurrence in `output`. A proper substring of a value is left as-is.
pub fn redact(output: &str, values: &[&str]) -> String {
    let mut vals: Vec<&str> = values.iter().copied().filter(|v| !v.is_empty()).collect();
    vals.sort_by_key(|v| std::cmp::Reverse(v.len()));
    vals.dedup();
    let mut out = output.to_string();
    for v in vals {
        out = out.replace(v, REPLACEMENT);
    }
    out
}
