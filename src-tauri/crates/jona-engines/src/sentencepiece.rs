//! SentencePiece `.model` parsing, without linking the C++ library.

use std::path::Path;

#[derive(prost::Message)]
pub struct ModelProto {
    #[prost(message, repeated, tag = "1")]
    pub pieces: Vec<SentencePieceProto>,
}

#[derive(prost::Message)]
pub struct SentencePieceProto {
    #[prost(string, optional, tag = "1")]
    pub piece: Option<String>,
    #[prost(float, optional, tag = "2")]
    pub score: Option<f32>,
}

pub fn read_model(path: &Path) -> Result<ModelProto, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("Failed to read SentencePiece model: {e}"))?;
    prost::Message::decode(data.as_slice())
        .map_err(|e| format!("Failed to parse SentencePiece protobuf: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    #[test]
    fn reads_pieces_and_scores_in_order() {
        let model = ModelProto {
            pieces: vec![
                SentencePieceProto { piece: Some("<unk>".into()), score: Some(0.0) },
                SentencePieceProto { piece: Some("\u{2581}le".into()), score: Some(-2.5) },
            ],
        };
        let path = std::env::temp_dir().join(format!("jona-spm-{}.model", std::process::id()));
        std::fs::write(&path, model.encode_to_vec()).unwrap();

        let read = read_model(&path);
        std::fs::remove_file(&path).unwrap();
        let read = read.unwrap();

        assert_eq!(read.pieces[1].piece.as_deref(), Some("\u{2581}le"));
        assert_eq!(read.pieces[1].score, Some(-2.5));
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(read_model(Path::new("/nonexistent/t.model")).is_err());
    }
}
