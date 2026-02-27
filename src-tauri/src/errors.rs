use crate::engines::EngineError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Engine error: {0}")]
    Engine(#[from] EngineError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
