//! Per-profile settings, and the user's own API keys.
//!
//! ADR-0027: **no TMDB key ever ships with this application, in any form, on any
//! channel.** Each user optionally supplies their own, per profile, under their own
//! acceptance of TMDB's terms, and may remove it at any time. Nothing here has a
//! default, a fallback, or a build-time constant, and there is deliberately no way to
//! ask this module for "the" key — only for a particular profile's.

use crate::db::{Db, DbError};
use crate::secrets;

/// The settings key a profile's TMDB key is stored under.
const TMDB_KEY: &str = "tmdb.api_key";

/// Whether a profile can reach TMDB.
///
/// **`Absent` is not an error and not an empty string.** It is the state the
/// application is designed around: `SPEC.md` §9.4's typographic treatment, which
/// ADR-0027 requires to be "genuinely beautiful rather than a fallback". Making this an
/// enum rather than an `Option<String>` is the point — a caller cannot reach the key
/// without writing down what it does when there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmdbAccess {
    Configured(String),
    Absent,
}

impl TmdbAccess {
    pub fn key(&self) -> Option<&str> {
        match self {
            TmdbAccess::Configured(key) => Some(key),
            TmdbAccess::Absent => None,
        }
    }

    pub fn is_configured(&self) -> bool {
        matches!(self, TmdbAccess::Configured(_))
    }
}

/// Why a key was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("a TMDB key cannot be empty")]
    Empty,
    #[error("that does not look like a TMDB key")]
    Malformed,
    #[error("the key could not be protected for storage on this machine")]
    NotProtectable,
}

/// TMDB v3 keys are 32 lowercase hexadecimal characters.
///
/// Validated on the way in, because the alternative is that a user pastes their
/// *username*, sees artwork silently never appear, and has no way to tell whether the
/// key or the network is at fault. Rejecting it at the point of entry is the only place
/// the error can be attributed correctly.
fn looks_like_a_tmdb_key(candidate: &str) -> bool {
    candidate.len() == 32 && candidate.chars().all(|c| c.is_ascii_hexdigit())
}

pub struct CredentialRepository<'a> {
    db: &'a Db,
}

impl<'a> CredentialRepository<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Store a profile's own TMDB key.
    pub async fn set_tmdb_key(&self, profile_id: i64, key: &str) -> Result<(), DbError> {
        let key = key.trim();
        if key.is_empty() {
            return Err(DbError::Invalid(KeyError::Empty.to_string()));
        }
        if !looks_like_a_tmdb_key(key) {
            return Err(DbError::Invalid(KeyError::Malformed.to_string()));
        }
        // If Windows will not protect it, the key is NOT stored. Falling back to
        // plaintext would quietly defeat the reason this is wrapped at all — see
        // `crate::secrets` — and a user who thinks their key is protected when it is
        // not is worse off than one who is told it could not be saved.
        let Some(wrapped) = secrets::wrap(key) else {
            return Err(DbError::Invalid(KeyError::NotProtectable.to_string()));
        };

        sqlx::query(
            "INSERT INTO profile_settings (profile_id, key, value, is_secret, updated_at)
             VALUES (?, ?, ?, 1, datetime('now'))
             ON CONFLICT (profile_id, key)
             DO UPDATE SET value = excluded.value, is_secret = 1, updated_at = datetime('now')",
        )
        .bind(profile_id)
        .bind(TMDB_KEY)
        .bind(wrapped)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// What this profile can reach.
    ///
    /// A stored blob that will not unwrap — a copied data folder, a different Windows
    /// account — reads as `Absent` rather than failing. The user re-enters their key and
    /// everything else keeps working, which is the behaviour ADR-0027 asks for.
    pub async fn tmdb_access(&self, profile_id: i64) -> Result<TmdbAccess, DbError> {
        let stored: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT value FROM profile_settings WHERE profile_id = ? AND key = ?",
        )
        .bind(profile_id)
        .bind(TMDB_KEY)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(match stored.as_deref().and_then(secrets::unwrap) {
            Some(key) if !key.is_empty() => TmdbAccess::Configured(key),
            _ => TmdbAccess::Absent,
        })
    }

    /// Remove a profile's key, and everything fetched with it.
    ///
    /// ADR-0027 point 3: removing the key is a supported action, and **cached artwork
    /// obtained under the old key is discarded rather than retained**. Leaving the cache
    /// behind would mean "remove my key" visibly did nothing — the posters would still
    /// be there — which is the opposite of what the user asked for.
    ///
    /// Returns `(key removed, cached responses discarded)`.
    pub async fn clear_tmdb_key(&self, profile_id: i64) -> Result<(bool, u64), DbError> {
        let mut tx = self.db.pool().begin().await?;

        let removed = sqlx::query("DELETE FROM profile_settings WHERE profile_id = ? AND key = ?")
            .bind(profile_id)
            .bind(TMDB_KEY)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        // The cache is keyed by URL and tagged by source, which is exactly what makes
        // this possible without knowing which rows belong to which profile.
        let discarded = sqlx::query("DELETE FROM http_cache WHERE source = 'tmdb'")
            .execute(&mut *tx)
            .await?
            .rows_affected();

        tx.commit().await?;
        Ok((removed > 0, discarded))
    }

    /// A non-secret per-profile setting.
    pub async fn set(&self, profile_id: i64, key: &str, value: &str) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO profile_settings (profile_id, key, value, is_secret, updated_at)
             VALUES (?, ?, ?, 0, datetime('now'))
             ON CONFLICT (profile_id, key)
             DO UPDATE SET value = excluded.value, is_secret = 0, updated_at = datetime('now')",
        )
        .bind(profile_id)
        .bind(key)
        .bind(value.as_bytes())
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Read a non-secret setting.
    ///
    /// Refuses to return a secret. Without that check a renamed or mistyped key would
    /// hand ciphertext to a caller expecting a string, and the first thing to notice
    /// would be a failing API call a long way from here.
    pub async fn get(&self, profile_id: i64, key: &str) -> Result<Option<String>, DbError> {
        let row: Option<(Vec<u8>, i64)> = sqlx::query_as(
            "SELECT value, is_secret FROM profile_settings WHERE profile_id = ? AND key = ?",
        )
        .bind(profile_id)
        .bind(key)
        .fetch_optional(self.db.pool())
        .await?;

        match row {
            Some((_, 1)) => Err(DbError::Invalid(format!(
                "{key:?} is a secret and must be read through its own accessor"
            ))),
            Some((value, _)) => Ok(String::from_utf8(value).ok()),
            None => Ok(None),
        }
    }

    /// How many profiles have supplied a key. For the settings screen, and for making
    /// sure the answer is never "one, by default".
    pub async fn profiles_with_a_key(&self) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM profile_settings WHERE key = ? AND is_secret = 1",
        )
        .bind(TMDB_KEY)
        .fetch_one(self.db.pool())
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_32_character_hex_string_looks_like_a_key() {
        assert!(looks_like_a_tmdb_key("abcdef0123456789abcdef0123456789"));
        assert!(looks_like_a_tmdb_key("ABCDEF0123456789ABCDEF0123456789"));
        // The realistic mistakes: a username, a v4 bearer token, a truncated paste.
        assert!(!looks_like_a_tmdb_key("my-tmdb-username"));
        assert!(!looks_like_a_tmdb_key("abcdef0123456789abcdef012345678"));
        assert!(!looks_like_a_tmdb_key("abcdef0123456789abcdef0123456789a"));
        assert!(!looks_like_a_tmdb_key(
            "eyJhbGciOiJIUzI1NiJ9.abcdefghijklmn"
        ));
        assert!(!looks_like_a_tmdb_key(""));
    }

    #[test]
    fn absence_is_a_state_a_caller_cannot_ignore() {
        assert_eq!(TmdbAccess::Absent.key(), None);
        assert!(!TmdbAccess::Absent.is_configured());
        let configured = TmdbAccess::Configured("abcdef0123456789abcdef0123456789".into());
        assert_eq!(configured.key(), Some("abcdef0123456789abcdef0123456789"));
        assert!(configured.is_configured());
    }
}
