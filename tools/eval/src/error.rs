//! One error type for every harness, so `main` can treat them alike.

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("{0}: {1}")]
    Fixture(String, String),
    #[error("{0}")]
    Missing(String),
    #[error(transparent)]
    Db(#[from] sinephile_persistence::DbError),
    #[error("database: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Artefact(#[from] sinephile_embedding::ArtefactError),
    #[error(transparent)]
    VectorIndex(#[from] sinephile_vector_index::VectorIndexError),
    #[error("{path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}
