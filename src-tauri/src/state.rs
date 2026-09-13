//! Application state.
//!
//! Small and boring on purpose: the hardware profile, the tier override, and the
//! database handle. No logic lives here — ADR-0022 keeps anything worth testing in
//! `crates/`, and this file is wiring.
//!
//! # The database arrives in Phase 5, not Phase 3
//!
//! It was supposed to land with the data layer and did not, which debt D26 recorded:
//! `CatalogueRepository`, `CredentialRepository` and the artwork cache were reachable
//! from tests and from `tools/ingest`, and unreachable from the running application.
//! Search is the first feature that cannot exist without it.
//!
//! **Opening it is deliberately not on the startup path.** `SPEC.md` §2.3 budgets cold
//! start at under 2 s on this tier, Phase 1 measured 515/660 ms, and a synchronous open
//! would spend some of that before the window appears. The handle is therefore created
//! on a background task and installed when it is ready; anything that needs it before
//! then gets `None` and says so, which is the same contract
//! `CatalogueRepository::readiness` already has for a half-built catalogue.

use std::sync::{Arc, RwLock};

use sinephile_persistence::Db;
use sinephile_search_engine::Engine;

use crate::tiers::{self, HardwareProfile, Tier};

pub struct AppState {
    inner: RwLock<Inner>,
    /// `None` until the background open completes. Never blocks a caller.
    db: RwLock<Option<Arc<Db>>>,
    /// The search engine, once its model and index are loaded.
    ///
    /// Behind a `tokio::Mutex` because `Engine::search` needs `&mut` — the embedder owns
    /// an ONNX session and one forward pass at a time is the honest model of it — and
    /// because it is held across awaits, which a `std` mutex may not be.
    engine: RwLock<Option<Arc<tokio::sync::Mutex<Engine>>>>,
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
            db: RwLock::new(None),
            engine: RwLock::new(None),
        }
    }

    /// Install the database handle once it has opened.
    pub fn set_db(&self, db: Arc<Db>) {
        *self.db.write().expect("db lock poisoned") = Some(db);
    }

    /// Install the search engine once its model and index are loaded.
    pub fn set_engine(&self, engine: Engine) {
        *self.engine.write().expect("engine lock poisoned") =
            Some(Arc::new(tokio::sync::Mutex::new(engine)));
    }

    /// The search engine, if it is ready.
    ///
    /// `None` while the model is still loading — about 140 ms after launch on this
    /// machine. A search that arrives first is told the catalogue is not ready yet,
    /// which is the truth and is what the readiness surface exists to express.
    pub fn engine(&self) -> Option<Arc<tokio::sync::Mutex<Engine>>> {
        self.engine.read().expect("engine lock poisoned").clone()
    }

    /// The database, if it has finished opening.
    ///
    /// A caller that arrives first gets `None` rather than a stall. On this machine the
    /// open takes single-digit milliseconds, so the window is small — but "small" is not
    /// "never", and a UI thread waiting on a disk is how a splash screen becomes a hang.
    pub fn db(&self) -> Option<Arc<Db>> {
        self.db.read().expect("db lock poisoned").clone()
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
