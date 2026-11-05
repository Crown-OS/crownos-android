const KIB_PER_MIB: f64 = 1024.0;
const BYTES_PER_MIB: f64 = KIB_PER_MIB * 1024.0;
const PERCENT: f64 = 100.0;

pub fn tenths(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

pub fn hundredths(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub fn mib_from_kib(kib: f64) -> f64 {
    hundredths(kib / KIB_PER_MIB)
}

pub fn mib_from_bytes(bytes: u64) -> f64 {
    hundredths(bytes as f64 / BYTES_PER_MIB)
}

/// `part` as a percentage of `whole`, to a hundredth.
pub fn percent(part: f64, whole: f64) -> f64 {
    hundredths(part / whole * PERCENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_and_rounds() {
        assert_eq!(mib_from_kib(51_200.0), 50.0);
        assert_eq!(mib_from_kib(-1_536.0), -1.5);
        assert_eq!(mib_from_bytes(1_258_291_200), 1200.0);
        assert_eq!(percent(1.0, 3.0), 33.33);
        assert_eq!(tenths(0.06), 0.1);
    }
}
