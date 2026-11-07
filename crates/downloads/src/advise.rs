//! Stream or download? (SPEC.md Phase 13.) One recommendation and one readable sentence,
//! from four signals the author asked for:
//!
//! 1. **Will a stream stall?** The swarm's measured throughput against the bitrate the file
//!    needs, as a probability: the spread is what was measured, widened for a thin swarm,
//!    where one peer leaving takes a large share with it.
//! 2. **Disk, and copies already here.** A copy on this PC wins outright; not enough free
//!    space rules a download out.
//! 3. **The network by hour.** If this connection is much faster at night and the file is
//!    large, tonight is the honest suggestion.
//! 4. **Taste.** A likely favourite the viewer will rewatch is worth the disk; something
//!    being sampled is not. (The taste model is Phase 15's; until then, neutral.)
//!
//! The other option is always one click away; this only says which is better and why.

/// The swarm, as measured for a few seconds after the film's page opened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swarm {
    /// Mean download rate, bytes per second.
    pub throughput: f64,
    /// Its standard deviation over the samples, bytes per second.
    pub spread: f64,
    pub peers: u32,
}

/// What the taste model thinks (Phase 15); `neutral()` until it exists.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Taste {
    /// 0 to 1: how likely this is to be a favourite.
    pub favourite: f64,
    pub rewatch: bool,
}

impl Taste {
    pub fn neutral() -> Self {
        Self {
            favourite: 0.5,
            rewatch: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Inputs {
    pub file_bytes: u64,
    pub runtime_seconds: f64,
    /// How many seconds the player buffers before it starts.
    pub buffer_seconds: f64,
    pub swarm: Option<Swarm>,
    pub local_copy: bool,
    pub free_disk_bytes: u64,
    /// Mean download rate per hour of the day (index 0 = midnight), bytes per second;
    /// `None` where nothing has been measured.
    pub hourly: [Option<f64>; 24],
    pub hour_now: usize,
    pub taste: Taste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    PlayLocal,
    Stream,
    Download,
    DownloadTonight,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Advice {
    pub choice: Choice,
    /// One sentence, for the viewer.
    pub sentence: String,
    /// The estimated chance a stream pauses to buffer, 0 to 1 (`None` without a swarm).
    pub stall: Option<f64>,
}

/// Headroom over the rate a stream needs, for a swarm that slows a little as it goes.
pub const HEADROOM: f64 = 1.05;
/// The buffer the app's player fills before it starts, in seconds.
pub const PLAYER_BUFFER: f64 = 10.0;
/// Below this chance of a stall, streaming is recommended.
pub const STREAM_IF_STALL_BELOW: f64 = 0.10;
/// Disk left free after a download, so the system is never filled.
pub const DISK_MARGIN: u64 = 2 * 1024 * 1024 * 1024;
/// "Large" for the overnight suggestion.
pub const LARGE: u64 = 4 * 1024 * 1024 * 1024;

/// The standard normal's cumulative distribution (Abramowitz and Stegun 26.2.17; error
/// under 7.5e-8, which is far finer than the inputs).
pub fn normal_cdf(z: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.231_641_9 * z.abs());
    let poly = t
        * (0.319_381_530
            + t * (-0.356_563_782
                + t * (1.781_477_937 + t * (-1.821_255_978 + t * 1.330_274_429))));
    let tail = (-z * z / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt() * poly;
    if z >= 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

/// How much of the rate a thin swarm's fragility is taken to add to the spread, before
/// dividing by the root of the peer count: a tenth with one peer.
pub const THIN_SWARM: f64 = 0.10;

/// The rate a stream needs so the buffer never runs dry: the film's bytes, delivered
/// over its runtime plus the time the buffer was filling. A small shortfall on a short
/// film is covered by the buffer; over two hours it is not.
pub fn needed_rate(file_bytes: u64, runtime_seconds: f64, buffer_seconds: f64) -> f64 {
    file_bytes as f64 / (runtime_seconds.max(1.0) + buffer_seconds.max(0.0))
}

/// The chance the swarm delivers less than `needed` (with headroom): throughput is taken
/// as normal, with the measured spread widened by the thin-swarm allowance.
pub fn stall_probability(needed: f64, swarm: Swarm) -> f64 {
    if swarm.throughput <= 0.0 {
        return 1.0;
    }
    let thin = swarm.throughput * THIN_SWARM / f64::from(swarm.peers.max(1)).sqrt();
    let spread = (swarm.spread * swarm.spread + thin * thin).sqrt().max(1.0);
    normal_cdf((needed * HEADROOM - swarm.throughput) / spread)
}

fn size_words(bytes: u64) -> String {
    let gib = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gib >= 1.0 {
        format!("{gib:.1} GB")
    } else {
        format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn rate_words(bps: f64) -> String {
    format!("{:.1} MB/s", bps / (1024.0 * 1024.0))
}

fn minutes_words(seconds: f64) -> String {
    let m = (seconds / 60.0).ceil().max(1.0) as u64;
    if m == 1 {
        "about a minute".to_string()
    } else if m >= 90 {
        format!("about {:.1} hours", m as f64 / 60.0)
    } else {
        format!("about {m} minutes")
    }
}

/// The best hours for this connection (1 to 6 in the morning), if measured.
fn night(hourly: &[Option<f64>; 24]) -> Option<f64> {
    let rates: Vec<f64> = hourly[1..=6].iter().flatten().copied().collect();
    (!rates.is_empty()).then(|| rates.iter().sum::<f64>() / rates.len() as f64)
}

pub fn advise(i: &Inputs) -> Advice {
    if i.local_copy {
        return Advice {
            choice: Choice::PlayLocal,
            sentence: "You already have it on this PC, so it plays straight away.".into(),
            stall: None,
        };
    }
    // What the sentence quotes: the film's own bitrate.
    let needed = i.file_bytes as f64 / i.runtime_seconds.max(1.0);
    let must = needed_rate(i.file_bytes, i.runtime_seconds, i.buffer_seconds);
    let stall = i.swarm.map(|s| stall_probability(must, s));
    let room = i.free_disk_bytes >= i.file_bytes.saturating_add(DISK_MARGIN);

    let Some(swarm) = i.swarm.filter(|s| s.peers > 0) else {
        return Advice {
            choice: Choice::Download,
            sentence:
                "Nobody is sharing it right now, so it is queued to download when someone does."
                    .into(),
            stall,
        };
    };
    let p = stall.unwrap_or(1.0);

    if !room {
        let sentence = if p < STREAM_IF_STALL_BELOW {
            format!(
                "There isn't room to keep it ({} needed, {} free), and the swarm is fast enough to stream it.",
                size_words(i.file_bytes),
                size_words(i.free_disk_bytes)
            )
        } else {
            format!(
                "There isn't room to keep it ({} needed, {} free), so stream it, though it may pause to buffer.",
                size_words(i.file_bytes),
                size_words(i.free_disk_bytes)
            )
        };
        return Advice {
            choice: Choice::Stream,
            sentence,
            stall,
        };
    }

    if p < STREAM_IF_STALL_BELOW {
        if i.taste.favourite >= 0.8 && i.taste.rewatch {
            return Advice {
                choice: Choice::Download,
                sentence: "It streams smoothly, but it looks like a favourite you'll watch again, so it's worth keeping."
                    .into(),
                stall,
            };
        }
        return Advice {
            choice: Choice::Stream,
            sentence: format!(
                "The swarm is fast enough to stream it without pausing ({} against the {} it needs).",
                rate_words(swarm.throughput),
                rate_words(needed)
            ),
            stall,
        };
    }

    let now = i.hourly[i.hour_now % 24].unwrap_or(swarm.throughput);
    if i.file_bytes >= LARGE {
        if let Some(n) = night(&i.hourly) {
            if n >= now * 1.5 {
                return Advice {
                    choice: Choice::DownloadTonight,
                    sentence: format!(
                        "It's too slow to stream now, and this connection is {:.0} times faster overnight, so it can be ready by morning.",
                        n / now.max(1.0)
                    ),
                    stall,
                };
            }
        }
    }
    Advice {
        choice: Choice::Download,
        sentence: format!(
            "It would pause to buffer if streamed ({} against the {} it needs), so download it first, {}.",
            rate_words(swarm.throughput),
            rate_words(needed),
            minutes_words(i.file_bytes as f64 / swarm.throughput)
        ),
        stall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: f64 = 1024.0 * 1024.0;
    const GB: u64 = 1024 * 1024 * 1024;

    fn base() -> Inputs {
        Inputs {
            // 2 GB over 100 minutes: about 0.34 MB/s.
            file_bytes: 2 * GB,
            runtime_seconds: 6000.0,
            buffer_seconds: PLAYER_BUFFER,
            swarm: Some(Swarm {
                throughput: 3.0 * MB,
                spread: 0.3 * MB,
                peers: 20,
            }),
            local_copy: false,
            free_disk_bytes: 200 * GB,
            hourly: [None; 24],
            hour_now: 20,
            taste: Taste::neutral(),
        }
    }

    /// One sentence: ends with a full stop, no other sentence break, no line break.
    fn one_sentence(s: &str) -> bool {
        s.ends_with('.') && !s[..s.len() - 1].contains(". ") && !s.contains('\n') && s.len() <= 160
    }

    #[test]
    fn the_normal_cdf_is_right_where_it_matters() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-7);
        assert!((normal_cdf(1.96) - 0.975).abs() < 1e-4);
        assert!((normal_cdf(-1.0) - 0.1587).abs() < 1e-4);
    }

    #[test]
    fn a_fast_healthy_swarm_streams() {
        let a = advise(&base());
        assert_eq!(a.choice, Choice::Stream);
        assert!(a.stall.unwrap() < 0.01);
        assert!(one_sentence(&a.sentence), "{}", a.sentence);
    }

    #[test]
    fn a_slow_swarm_says_download_and_how_long() {
        let mut i = base();
        i.swarm = Some(Swarm {
            throughput: 0.3 * MB,
            spread: 0.05 * MB,
            peers: 4,
        });
        let a = advise(&i);
        assert_eq!(a.choice, Choice::Download);
        assert!(a.sentence.contains("about"), "{}", a.sentence);
        assert!(one_sentence(&a.sentence), "{}", a.sentence);
    }

    #[test]
    fn a_fast_but_thin_swarm_is_not_trusted() {
        // A rate that swings, or one peer's rate only just above the need, is not trusted;
        // the same mean, steady and from forty peers, is.
        let mut i = base();
        i.swarm = Some(Swarm {
            throughput: 0.5 * MB,
            spread: 0.3 * MB,
            peers: 1,
        });
        assert_eq!(advise(&i).choice, Choice::Download);
        i.swarm = Some(Swarm {
            throughput: 0.5 * MB,
            spread: 0.02 * MB,
            peers: 40,
        });
        assert_eq!(advise(&i).choice, Choice::Stream);
    }

    #[test]
    fn a_short_film_is_covered_by_the_buffer_and_a_long_one_is_not() {
        // 0.9 of the bitrate: 20 seconds after a 6-second buffer plays through (23.4 MB
        // arrive for 20 needed); two hours does not.
        let swarm = Swarm {
            throughput: 0.9 * MB,
            spread: 0.01 * MB,
            peers: 3,
        };
        let short = needed_rate((20.0 * MB) as u64, 20.0, 6.0);
        assert!(stall_probability(short, swarm) < 0.1);
        let long = needed_rate((7200.0 * MB) as u64, 7200.0, 3.0);
        assert!(stall_probability(long, swarm) > 0.9);
    }

    #[test]
    fn durations_read_as_words() {
        assert_eq!(minutes_words(20.0), "about a minute");
        assert_eq!(minutes_words(600.0), "about 10 minutes");
        assert_eq!(minutes_words(3.0 * 3600.0), "about 3.0 hours");
    }

    #[test]
    fn a_copy_on_this_pc_wins_outright() {
        let mut i = base();
        i.local_copy = true;
        assert_eq!(advise(&i).choice, Choice::PlayLocal);
    }

    #[test]
    fn no_room_means_stream_and_says_so() {
        let mut i = base();
        i.free_disk_bytes = 3 * GB;
        let a = advise(&i);
        assert_eq!(a.choice, Choice::Stream);
        assert!(a.sentence.contains("room"), "{}", a.sentence);
        assert!(one_sentence(&a.sentence), "{}", a.sentence);
    }

    #[test]
    fn a_large_file_on_a_slow_evening_waits_for_a_fast_night() {
        let mut i = base();
        i.file_bytes = 20 * GB;
        i.swarm = Some(Swarm {
            throughput: 1.0 * MB,
            spread: 0.1 * MB,
            peers: 10,
        });
        i.hourly[20] = Some(1.0 * MB);
        for h in 1..=6 {
            i.hourly[h] = Some(5.0 * MB);
        }
        let a = advise(&i);
        assert_eq!(a.choice, Choice::DownloadTonight);
        assert!(one_sentence(&a.sentence), "{}", a.sentence);
        // Without the faster night, an ordinary download.
        i.hourly = [None; 24];
        assert_eq!(advise(&i).choice, Choice::Download);
    }

    #[test]
    fn a_likely_favourite_is_worth_the_disk() {
        let mut i = base();
        i.taste = Taste {
            favourite: 0.9,
            rewatch: true,
        };
        let a = advise(&i);
        assert_eq!(a.choice, Choice::Download);
        assert!(one_sentence(&a.sentence), "{}", a.sentence);
    }

    #[test]
    fn nobody_sharing_queues_it() {
        let mut i = base();
        i.swarm = Some(Swarm {
            throughput: 0.0,
            spread: 0.0,
            peers: 0,
        });
        let a = advise(&i);
        assert_eq!(a.choice, Choice::Download);
        assert!(one_sentence(&a.sentence), "{}", a.sentence);
    }
}
