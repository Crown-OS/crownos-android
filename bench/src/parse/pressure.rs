use std::collections::BTreeMap;

/// The averages of a `/proc/pressure/*` file, as `some_avg10` → 1.21. Totals are left out.
pub fn averages(text: &str) -> BTreeMap<String, f64> {
    text.lines()
        .filter_map(|line| line.split_once(' '))
        .flat_map(|(kind, fields)| {
            fields.split_whitespace().filter_map(move |field| {
                let (name, value) = field.split_once('=')?;
                if name == "total" {
                    return None;
                }
                Some((format!("{kind}_{name}"), value.parse().ok()?))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_memory_pressure() {
        let pressure = averages(include_str!("../../fixtures/pressure-memory"));
        assert_eq!(pressure.len(), 6);
        assert_eq!(pressure.get("some_avg10"), Some(&1.21));
        assert_eq!(pressure.get("full_avg300"), Some(&1.48));
        assert!(!pressure.contains_key("some_total"));
    }
}
