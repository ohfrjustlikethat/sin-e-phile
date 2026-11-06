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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use sinephile_metadata_api::HttpTransport;
use sinephile_persistence::Db;
use sinephile_search_engine::Engine;
use source_protocol::Manifest;
use url::Url;

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
    /// The two HTTP clients sources use (Phase 6). Built once: a client holds a
    /// connection pool, and a new one per search would reconnect every time.
    pub http: Arc<HttpTransport>,
    /// For addresses a user supplied — addons and catalogues. No redirects.
    pub untrusted_http: Arc<HttpTransport>,
    /// Manifests shown on the confirm screen and not yet installed, by the URL the user
    /// pasted. Install takes the manifest from HERE, so what is installed is exactly
    /// what was shown — not whatever the address serves a second time.
    pending_addons: Mutex<HashMap<String, (Url, Manifest)>>,
    /// Where `data/` is, once the startup task has found it.
    data_dir: RwLock<Option<PathBuf>>,
    /// The torrent engine and its streaming server, started the first time anything
    /// needs them — never on the cold-start path (SPEC.md §2.3).
    torrent: tokio::sync::OnceCell<Arc<TorrentRuntime>>,
    /// The player and its video window, created on the first play (Phase 8).
    player: RwLock<Option<Arc<crate::commands::player::PlayerRuntime>>>,
}

pub struct TorrentRuntime {
    pub engine: Arc<sinephile_torrent::Engine>,
    pub server: sinephile_torrent::StreamServer,
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
            http: Arc::new(HttpTransport::new()),
            untrusted_http: Arc::new(HttpTransport::untrusted()),
            pending_addons: Mutex::new(HashMap::new()),
            data_dir: RwLock::new(None),
            torrent: tokio::sync::OnceCell::new(),
            player: RwLock::new(None),
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

    pub fn player(&self) -> Option<Arc<crate::commands::player::PlayerRuntime>> {
        self.player.read().expect("player lock poisoned").clone()
    }

    pub fn set_player(&self, rt: Arc<crate::commands::player::PlayerRuntime>) {
        *self.player.write().expect("player lock poisoned") = Some(rt);
    }

    pub fn set_data_dir(&self, dir: PathBuf) {
        *self.data_dir.write().expect("data dir lock poisoned") = Some(dir);
    }

    /// The torrent runtime, started on first call with the stored settings. The
    /// engine's rules are in `crates/torrent`; this only builds it once.
    pub async fn torrent(&self) -> Result<Arc<TorrentRuntime>, String> {
        self.torrent
            .get_or_try_init(|| async {
                let dir = self
                    .data_dir
                    .read()
                    .expect("data dir lock poisoned")
                    .clone()
                    .ok_or("the data folder is not ready yet")?;
                let db = self.db().ok_or("the library is still opening")?;
                let stored = sinephile_persistence::repositories::ProfileRepository::new(&db)
                    .setting(sinephile_torrent::settings::SETTINGS_KEY)
                    .await
                    .map_err(|e| e.to_string())?;
                let settings =
                    sinephile_torrent::settings::TorrentSettings::parse(stored.as_deref());
                let config = settings.apply(sinephile_torrent::EngineConfig::new(dir));
                let engine = sinephile_torrent::Engine::start(config)
                    .await
                    .map_err(|e| e.to_string())?;
                let server = sinephile_torrent::StreamServer::start()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(Arc::new(TorrentRuntime {
                    engine: Arc::new(engine),
                    server,
                }))
            })
            .await
            .cloned()
    }

    /// The runtime if it has been started, without starting it.
    pub fn torrent_if_started(&self) -> Option<Arc<TorrentRuntime>> {
        self.torrent.get().cloned()
    }

    /// Remember a manifest the user is being asked to confirm.
    pub fn hold_addon(&self, pasted: &str, url: Url, manifest: Manifest) {
        self.pending_addons
            .lock()
            .expect("pending lock poisoned")
            .insert(pasted.to_string(), (url, manifest));
    }

    /// The manifest shown for this address, taken so it cannot be installed twice.
    pub fn take_addon(&self, pasted: &str) -> Option<(Url, Manifest)> {
        self.pending_addons
            .lock()
            .expect("pending lock poisoned")
            .remove(pasted)
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
