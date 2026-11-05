use std::cmp::Ordering;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Part<'a> {
    Number(u64),
    Text(&'a str),
}

fn parts(version: &str) -> impl Iterator<Item = Part<'_>> {
    version
        .split('.')
        .map(|part| part.parse().map_or(Part::Text(part), Part::Number))
}

/// Orders dotted versions such as `2026.11.0` by their numeric parts.
pub fn compare(left: &str, right: &str) -> Ordering {
    parts(left).cmp(parts(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_parts_compare_as_numbers() {
        assert_eq!(compare("2026.11.0", "2026.9.3"), Ordering::Greater);
        assert_eq!(compare("2026.11", "2026.11.1"), Ordering::Less);
        assert_eq!(compare("2026.11.0", "2026.11.0"), Ordering::Equal);
    }
}
