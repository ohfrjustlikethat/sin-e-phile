//! Hardware capability tiers — SPEC.md §8.
//!
//! **A standalone crate, and deliberately free of any Tauri dependency**
//! (ADR-0022). Test binaries that link Tauri cannot launch on Windows without a
//! side-by-side manifest that `cargo test` does not provide, so pure logic lives
//! in crates like this one where it can actually be tested.
//!
//! **This is the only module that looks at hardware.** §8's rule is explicit: no
//! feature checks hardware directly, everything goes through `Capability`. That
//! keeps the gating auditable in one place, and it means a manual override works
//! everywhere rather than in the places someone remembered to check.
//!
//! The other half of §8 matters as much: every gated feature must degrade to
//! something *good*, never to something broken or empty. Face recognition off
//! means the pause overlay shows the full cast list, beautifully — not an empty
//! panel.

use serde::{Deserialize, Serialize};
use specta::Type;
use sysinfo::System;

/// The three tiers from §8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// < 8 GB RAM, or no hardware decode, or <= 2 physical cores.
    Modest,
    /// 8-16 GB RAM, hardware decode present, >= 4 cores.
    Standard,
    /// >= 16 GB RAM, discrete GPU or strong iGPU, >= 6 cores.
    Capable,
}

impl Tier {
    pub fn as_number(self) -> u8 {
        match self {
            Tier::Modest => 0,
            Tier::Standard => 1,
            Tier::Capable => 2,
        }
    }
}

/// What a tier permits. Features ask about a `Capability`, never about hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Embed catalogue DOCUMENTS on device. Tier 1+.
    ///
    /// Note ADR-0015: embedding a QUERY is permitted on every tier and is
    /// deliberately not gated here. The asymmetry is scale — hundreds of
    /// thousands of long documents versus one ~30-token query.
    LocalDocumentEmbedding,
    /// VAD-based subtitle alignment. Tier 1+. Below that: hash match, duration
    /// heuristics, and a manual nudge that is remembered per file.
    VadSubtitleAlignment,
    /// Intro/credit detection across a season. Tier 1+.
    BingeDetection,
    /// Playback above 1080p by default. Tier 1+ (users may override).
    HighResPlayback,
    /// Face recognition in the pause overlay. Tier 2 only.
    FaceRecognition,
    /// Whisper-based subtitle generation. Tier 2 only.
    LocalTranscription,
    /// Background pre-embedding of the catalogue. Tier 2 only.
    BackgroundPreEmbedding,
    /// Full motion design: hover previews, background blur, parallax.
    FullMotion,
}

impl Tier {
    pub fn allows(self, cap: Capability) -> bool {
        use Capability::*;
        match cap {
            LocalDocumentEmbedding
            | VadSubtitleAlignment
            | BingeDetection
            | HighResPlayback
            | FullMotion => self >= Tier::Standard,
            FaceRecognition | LocalTranscription | BackgroundPreEmbedding => self == Tier::Capable,
        }
    }
}

// Ordering so `>= Tier::Standard` reads correctly. Derived ordering would follow
// declaration order, which happens to be right, but stating it is safer than
// depending on the order of an enum someone may reorder later.
impl PartialOrd for Tier {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Tier {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_number().cmp(&other.as_number())
    }
}

/// What detection actually found. Shown verbatim in Settings (§8: "in plain
/// language. This is a nice UI moment, not an apology").
/// Note the integer widths. Specta refuses to export u64/usize/i64 across the IPC
/// boundary because JavaScript numbers are f64 and would silently lose precision
/// above 2^53. u32 is not a workaround — it is the correct type here: 4 billion MB
/// is four petabytes of RAM, and nobody has 4 billion CPU cores.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HardwareProfile {
    pub total_memory_mb: u32,
    pub physical_cores: u32,
    pub logical_cores: u32,
    pub cpu_brand: String,
    pub gpu_name: Option<String>,
    pub hardware_decode: bool,
    /// The tier detection arrived at, before any override.
    pub detected_tier: Tier,
    /// The tier actually in force. Differs only when the user has overridden.
    pub effective_tier: Tier,
    pub overridden: bool,
}

/// Classify per §8's table.
///
/// Deliberately conservative: any single disqualifying signal drops the tier.
/// Being wrong downward costs a feature; being wrong upward costs a bad
/// experience on hardware that cannot carry it, and §2.3 is enforced against
/// Tier 0.
pub fn classify(mem_mb: u32, physical_cores: u32, hw_decode: bool, has_gpu: bool) -> Tier {
    let gb = mem_mb as f64 / 1024.0;

    if gb < 8.0 || !hw_decode || physical_cores <= 2 {
        return Tier::Modest;
    }
    if gb >= 16.0 && physical_cores >= 6 && has_gpu {
        return Tier::Capable;
    }
    Tier::Standard
}

/// Probe the machine.
pub fn detect() -> HardwareProfile {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let total_memory_mb = (sys.total_memory() / 1024 / 1024).min(u32::MAX as u64) as u32;
    let logical_cores = sys.cpus().len().max(1) as u32;
    let physical_cores = sys
        .physical_core_count()
        .map(|c| c.max(1) as u32)
        .unwrap_or(logical_cores);
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    let (gpu_name, hardware_decode, has_gpu) = probe_gpu();
    let detected_tier = classify(total_memory_mb, physical_cores, hardware_decode, has_gpu);

    HardwareProfile {
        total_memory_mb,
        physical_cores,
        logical_cores,
        cpu_brand,
        gpu_name,
        hardware_decode,
        detected_tier,
        effective_tier: detected_tier,
        overridden: false,
    }
}

/// GPU name, whether hardware decode is plausible, and whether the GPU is
/// substantial enough for Tier 2.
///
/// Windows-only by design (§2.4). Uses DXGI rather than WMI: WMI is slow enough
/// to be visible in a < 4 s cold-start budget, and DXGI is what actually
/// describes the adapter D3D11VA would use.
#[cfg(windows)]
fn probe_gpu() -> (Option<String>, bool, bool) {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG, DXGI_ADAPTER_FLAG_SOFTWARE,
    };

    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else {
            return (None, false, false);
        };

        let mut best: Option<(String, u64)> = None;
        let mut index = 0u32;
        while let Ok(adapter) = factory.EnumAdapters1(index) {
            index += 1;
            // windows 0.61 returns the descriptor rather than filling a pointer.
            let Ok(desc) = adapter.GetDesc1() else {
                continue;
            };
            // Skip the software renderer; it would falsely imply hardware decode.
            if DXGI_ADAPTER_FLAG(desc.Flags as i32) == DXGI_ADAPTER_FLAG_SOFTWARE {
                continue;
            }
            let name = String::from_utf16_lossy(&desc.Description)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            let vram = desc.DedicatedVideoMemory as u64;
            if best.as_ref().is_none_or(|(_, v)| vram > *v) {
                best = Some((name, vram));
            }
            let _ = adapter;
        }

        match best {
            Some((name, vram)) => {
                // Any non-software D3D11 adapter on a supported Windows build has
                // some hardware decode. Whether it decodes a SPECIFIC codec is a
                // Phase 8 question — mpv reports `hwdec-current` at play time, and
                // that is the honest answer. This is the coarse gate §8 needs.
                let hw_decode = true;
                // >= 2 GB dedicated VRAM stands in for "discrete GPU or strong
                // iGPU". Crude, but it separates a real GPU from a basic iGPU
                // without shipping a device database that would go stale.
                let substantial = vram >= 2 * 1024 * 1024 * 1024;
                (Some(name), hw_decode, substantial)
            }
            None => (None, false, false),
        }
    }
}

#[cfg(not(windows))]
fn probe_gpu() -> (Option<String>, bool, bool) {
    // SPEC.md §2.4: Windows only. This exists so `cargo test` runs elsewhere,
    // not as a portability layer.
    (None, false, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // §8's table, exercised at its boundaries. These are the cases where an
    // off-by-one silently gives a weak machine features it cannot carry.
    #[test]
    fn under_8gb_is_modest_regardless_of_everything_else() {
        assert_eq!(classify(7_500, 16, true, true), Tier::Modest);
    }

    #[test]
    fn no_hardware_decode_is_modest_regardless_of_everything_else() {
        assert_eq!(classify(64_000, 16, false, true), Tier::Modest);
    }

    #[test]
    fn two_or_fewer_physical_cores_is_modest() {
        assert_eq!(classify(32_000, 2, true, true), Tier::Modest);
        assert_eq!(classify(32_000, 3, true, true), Tier::Standard);
    }

    #[test]
    fn standard_needs_8gb_and_decode() {
        assert_eq!(classify(8_192, 4, true, false), Tier::Standard);
    }

    #[test]
    fn capable_needs_all_three_of_16gb_six_cores_and_a_gpu() {
        assert_eq!(classify(16_384, 6, true, true), Tier::Capable);
        assert_eq!(classify(16_384, 6, true, false), Tier::Standard, "no GPU");
        assert_eq!(
            classify(16_384, 5, true, true),
            Tier::Standard,
            "too few cores"
        );
        assert_eq!(
            classify(15_000, 8, true, true),
            Tier::Standard,
            "too little RAM"
        );
    }

    #[test]
    fn tier_2_only_capabilities_are_not_granted_below_tier_2() {
        for tier in [Tier::Modest, Tier::Standard] {
            assert!(!tier.allows(Capability::FaceRecognition));
            assert!(!tier.allows(Capability::LocalTranscription));
            assert!(!tier.allows(Capability::BackgroundPreEmbedding));
        }
        assert!(Tier::Capable.allows(Capability::FaceRecognition));
    }

    #[test]
    fn modest_gets_no_gated_capability_at_all() {
        use Capability::*;
        for cap in [
            LocalDocumentEmbedding,
            VadSubtitleAlignment,
            BingeDetection,
            HighResPlayback,
            FaceRecognition,
            LocalTranscription,
            BackgroundPreEmbedding,
            FullMotion,
        ] {
            assert!(!Tier::Modest.allows(cap), "Modest must not allow {cap:?}");
        }
    }

    #[test]
    fn tiers_order_by_capability_not_declaration() {
        assert!(Tier::Capable > Tier::Standard);
        assert!(Tier::Standard > Tier::Modest);
    }
}
