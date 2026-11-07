//! What the advisor needs from this machine: free disk space, and how fast this connection
//! is at each hour of the day (learned from downloads as they run).

use std::path::Path;

/// Free bytes on the drive holding `path`, for this user (quotas included). `None` if
/// Windows will not say.
pub fn free_space(path: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        // The nearest folder that exists: a downloads folder may not have been made yet.
        let mut at = path;
        while !at.exists() {
            at = at.parent()?;
        }
        let wide: Vec<u16> = at.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut available: u64 = 0;
        // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call; the
        // out-pointer is to a live u64, and the two totals we do not want are null, which
        // the API allows.
        let ok = unsafe {
            GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut available,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        (ok != 0).then_some(available)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

/// Mean download rate per hour of the day: (mean bytes per second, samples), index 0 being
/// midnight to one. Kept as a plain setting (JSON), so no migration is needed.
pub type Hourly = [(f64, u32); 24];

/// Fold one sample into its hour's running mean. Old samples fade (the count is capped),
/// so a connection that changes is learned again within a few days.
pub fn fold(mut hourly: Hourly, hour: usize, bytes_per_second: f64) -> Hourly {
    let (mean, n) = hourly[hour % 24];
    let n = (n + 1).min(200);
    hourly[hour % 24] = (mean + (bytes_per_second - mean) / f64::from(n), n);
    hourly
}

/// The advisor's view: an hour with no samples is unknown.
pub fn rates(hourly: &Hourly) -> [Option<f64>; 24] {
    let mut out = [None; 24];
    for (i, (mean, n)) in hourly.iter().enumerate() {
        if *n > 0 {
            out[i] = Some(*mean);
        }
    }
    out
}

pub fn to_json(hourly: &Hourly) -> String {
    let parts: Vec<String> = hourly
        .iter()
        .map(|(m, n)| format!("[{m:.0},{n}]"))
        .collect();
    format!("[{}]", parts.join(","))
}

pub fn from_json(text: &str) -> Hourly {
    let mut out: Hourly = [(0.0, 0); 24];
    if let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(text) {
        for (i, item) in items.iter().take(24).enumerate() {
            if let Some([m, n]) = item
                .as_array()
                .map(|a| [a.first().cloned(), a.get(1).cloned()])
            {
                out[i] = (
                    m.and_then(|v| v.as_f64()).unwrap_or(0.0),
                    n.and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                );
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_hour_learns_a_running_mean_and_forgets_slowly() {
        let mut h: Hourly = [(0.0, 0); 24];
        h = fold(h, 3, 100.0);
        h = fold(h, 3, 300.0);
        assert_eq!(h[3], (200.0, 2));
        assert_eq!(rates(&h)[3], Some(200.0));
        assert_eq!(rates(&h)[4], None, "unmeasured is unknown, not zero");
        for _ in 0..1000 {
            h = fold(h, 3, 1000.0);
        }
        assert!(
            h[3].0 > 990.0,
            "a changed connection is learned again: {}",
            h[3].0
        );
    }

    #[test]
    fn the_history_survives_a_round_trip_and_nonsense_reads_as_empty() {
        let h = fold([(0.0, 0); 24], 22, 5_000_000.0);
        let back = from_json(&to_json(&h));
        assert_eq!(back[22], (5_000_000.0, 1));
        assert_eq!(from_json("not json")[0], (0.0, 0));
    }

    #[test]
    fn the_free_space_of_this_drive_is_known() {
        let here = std::env::temp_dir()
            .join("sinephile-not-made-yet")
            .join("Downloads");
        let free = free_space(&here).expect("Windows says");
        assert!(free > 0);
    }
}
