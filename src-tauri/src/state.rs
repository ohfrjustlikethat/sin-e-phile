//! Application state.
//!
//! Small and boring on purpose. Phase 3 introduces the database and this grows;
//! until then it holds the hardware profile and the tier override.

use std::sync::RwLock;

use crate::tiers::{self, HardwareProfile, Tier};

pub struct AppState {
    inner: RwLock<Inner>,
}

struct Inner {
    detected: HardwareProfile,
    override_tier: Option<Tier>,
}

impl AppState {
    pub fn new() -> Self {
        let detected = tiers::detect();
        tracing::info!(
            tier = ?detected.detected_tier,
            memory_mb = detected.total_memory_mb,
            physical_cores = detected.physical_cores,
            gpu = ?detected.gpu_name,
            hardware_decode = detected.hardware_decode,
            "hardware detected"
        );
        Self {
            inner: RwLock::new(Inner {
                detected,
                override_tier: None,
            }),
        }
    }

    pub fn hardware_profile(&self) -> HardwareProfile {
        let g = self.inner.read().expect("state lock poisoned");
        let mut p = g.detected.clone();
        if let Some(t) = g.override_tier {
            p.effective_tier = t;
            p.overridden = true;
        }
        p
    }

    pub fn effective_tier(&self) -> Tier {
        let g = self.inner.read().expect("state lock poisoned");
        g.override_tier.unwrap_or(g.detected.detected_tier)
    }

    pub fn set_tier_override(&self, tier: Option<Tier>) -> HardwareProfile {
        {
            let mut g = self.inner.write().expect("state lock poisoned");
            g.override_tier = tier;
        }
        tracing::info!(?tier, "tier override changed");
        self.hardware_profile()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
