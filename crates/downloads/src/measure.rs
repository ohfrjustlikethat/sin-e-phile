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

/// The swarm as the advisor sees it, from rate samples (bytes per second) and the most peers
/// seen. The later half only: the first seconds are peers still connecting, which
/// understates a swarm (the same rule the advise eval was judged with).
pub fn swarm_from(samples: &[f64], peers: u32) -> Option<crate::advise::Swarm> {
    let later = &samples[samples.len() / 2..];
    if later.is_empty() {
        return None;
    }
    let n = later.len() as f64;
    let mean = later.iter().sum::<f64>() / n;
    let var = later.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
    Some(crate::advise::Swarm {
        throughput: mean,
        spread: var.sqrt(),
        peers,
    })
}

/// This PC's offset from UTC now, in seconds (local minus UTC), daylight saving included.
pub fn utc_offset_seconds() -> i64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::SYSTEMTIME;
        use windows_sys::Win32::System::SystemInformation::{GetLocalTime, GetSystemTime};
        // SAFETY: both write one SYSTEMTIME into memory we own.
        let (local, utc) = unsafe {
            let mut l: SYSTEMTIME = std::mem::zeroed();
            let mut u: SYSTEMTIME = std::mem::zeroed();
            GetLocalTime(&mut l);
            GetSystemTime(&mut u);
            (l, u)
        };
        let secs = |t: &SYSTEMTIME| {
            days_from_civil(i64::from(t.wYear), i64::from(t.wMonth), i64::from(t.wDay)) * 86_400
                + i64::from(t.wHour) * 3600
                + i64::from(t.wMinute) * 60
                + i64::from(t.wSecond)
        };
        // Rounded to the quarter hour: the two reads are a moment apart.
        ((secs(&local) - secs(&utc)) as f64 / 900.0).round() as i64 * 900
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's days-from-civil).
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The local hour of the day at `unix_seconds`.
pub fn local_hour(unix_seconds: i64, offset: i64) -> usize {
    ((unix_seconds + offset).rem_euclid(86_400) / 3600) as usize
}

/// When "tonight" starts: the next 01:00 local after `unix_seconds`, as Unix seconds.
pub fn tonight(unix_seconds: i64, offset: i64) -> i64 {
    let local = unix_seconds + offset;
    let day = local.div_euclid(86_400) * 86_400;
    let mut at = day + 3600;
    if at <= local {
        at += 86_400;
    }
    at - offset
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
    fn the_swarm_is_judged_on_its_later_samples() {
        // Connecting (0, 0) then a steady 100 and 300: the mean of the later half.
        let s = swarm_from(&[0.0, 0.0, 100.0, 300.0], 3).unwrap();
        assert_eq!((s.throughput, s.spread, s.peers), (200.0, 100.0, 3));
        assert_eq!(swarm_from(&[], 3), None);
    }

    #[test]
    fn tonight_is_the_next_one_in_the_morning_local() {
        // 2026-11-07T22:30:00Z.
        let at = 1_794_090_600;
        assert_eq!(crate::manager::iso_utc(at), "2026-11-07T22:30:00Z");
        // UTC: 01:00 tomorrow.
        assert_eq!(
            crate::manager::iso_utc(tonight(at, 0)),
            "2026-11-08T01:00:00Z"
        );
        // UTC+3 (01:30 local already): 01:00 local the next day, 22:00Z.
        assert_eq!(
            crate::manager::iso_utc(tonight(at, 3 * 3600)),
            "2026-11-08T22:00:00Z"
        );
        // UTC-5 (17:30 local): 01:00 local is 06:00Z.
        assert_eq!(
            crate::manager::iso_utc(tonight(at, -5 * 3600)),
            "2026-11-08T06:00:00Z"
        );
        assert_eq!(local_hour(at, 0), 22);
        assert_eq!(local_hour(at, 3 * 3600), 1);
    }

    #[test]
    fn this_pc_knows_its_offset_from_utc() {
        let o = utc_offset_seconds();
        assert!(o.abs() <= 14 * 3600 && o % 900 == 0, "{o}");
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
