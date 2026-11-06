//! A candidate builder shared by the fixture and property tests.

#![allow(dead_code)]

use source_protocol::{
    Health, Provenance, Quality, Resolution, SourceCandidate, Transport, VideoCodec,
};

pub const MB: u64 = 1_000_000;
pub const GB: u64 = 1_000_000_000;

pub struct C(pub SourceCandidate);

pub fn http(key: &str) -> C {
    C(SourceCandidate {
        key: key.into(),
        title: key.into(),
        year: None,
        transport: Transport::Http,
        quality: Quality::default(),
        languages: vec![],
        subtitles: vec![],
        codec: VideoCodec::Unknown,
        size_bytes: None,
        duration_seconds: None,
        health: Health::Direct,
        locator: None,
        provenance: Provenance::default(),
    })
}

pub fn swarm(key: &str, seeders: u32) -> C {
    let mut c = http(key);
    c.0.transport = Transport::P2p;
    c.0.health = Health::Swarm { seeders, peers: 0 };
    c
}

pub fn local(key: &str) -> C {
    let mut c = http(key);
    c.0.transport = Transport::Local;
    c
}

impl C {
    pub fn res(mut self, r: Resolution) -> Self {
        self.0.quality.resolution = r;
        self
    }
    pub fn hdr(mut self) -> Self {
        self.0.quality.hdr = true;
        self
    }
    pub fn codec(mut self, c: VideoCodec) -> Self {
        self.0.codec = c;
        self
    }
    pub fn size(mut self, bytes: u64) -> Self {
        self.0.size_bytes = Some(bytes);
        self
    }
    pub fn minutes(mut self, m: u32) -> Self {
        self.0.duration_seconds = Some(m * 60);
        self
    }
    pub fn audio(mut self, langs: &[&str]) -> Self {
        self.0.languages = langs.iter().map(|s| s.to_string()).collect();
        self
    }
    pub fn subs(mut self, langs: &[&str]) -> Self {
        self.0.subtitles = langs.iter().map(|s| s.to_string()).collect();
        self
    }
    pub fn titled(mut self, t: &str) -> Self {
        self.0.title = t.into();
        self
    }
    pub fn done(self) -> SourceCandidate {
        self.0
    }
}
