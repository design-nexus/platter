pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() || text == "0" || text.eq_ignore_ascii_case("max") {
        return Some(0);
    }
    let (number, unit) = text
        .char_indices()
        .find(|(_, c)| c.is_ascii_alphabetic())
        .map(|(i, _)| text.split_at(i))
        .unwrap_or((text, ""));
    let number = number.trim();
    let mult: u128 = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" => 1000,
        "kib" => 1024,
        "m" | "mb" => 1000_u128.pow(2),
        "mib" => 1024_u128.pow(2),
        "g" | "gb" => 1000_u128.pow(3),
        "gib" => 1024_u128.pow(3),
        "t" | "tb" => 1000_u128.pow(4),
        "tib" => 1024_u128.pow(4),
        _ => return None,
    };
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole
        .chars()
        .chain(fraction.chars())
        .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let scale = 10_u128.checked_pow(fraction.len().try_into().ok()?)?;
    let whole: u128 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction: u128 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().ok()?
    };
    let numerator = whole
        .checked_mul(scale)?
        .checked_add(fraction)?
        .checked_mul(mult)?;
    let value = numerator.checked_add(scale / 2)? / scale;
    u64::try_from(value).ok()
}

pub fn kernel_name(device: &str) -> &str {
    device.rsplit('/').next().unwrap_or(device)
}

pub fn c_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

pub fn with_nul(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

pub fn brief(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    line.chars().take(160).collect()
}

pub fn current_ids() -> (u32, u32) {
    unsafe { (libc::geteuid(), libc::getegid()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_round_trip() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(999), "999 B");
        assert_eq!(parse_size("1.5G"), Some(1_500_000_000));
        assert_eq!(parse_size("max"), Some(0));
        assert_eq!(parse_size("512M"), Some(512_000_000));
        assert_eq!(kernel_name("/dev/nvme0n1p2"), "nvme0n1p2");
        assert_eq!(c_string(b"UUID=abc\0junk"), "UUID=abc");
    }
}

#[cfg(test)]
mod size_regressions {
    use super::*;
    #[test]
    fn binary_units_and_boundaries() {
        assert_eq!(parse_size("1 GiB"), Some(1 << 30));
        assert_eq!(parse_size("1.5 MiB"), Some(1572864));
        assert_eq!(parse_size("18446744073709551615"), Some(u64::MAX));
        for invalid in [
            "NaN",
            "inf",
            "-1",
            "1e20",
            "18446744073709551616",
            "999999999999TiB",
            "1..2G",
            "+1",
        ] {
            assert_eq!(parse_size(invalid), None, "{invalid}");
        }
    }
}
