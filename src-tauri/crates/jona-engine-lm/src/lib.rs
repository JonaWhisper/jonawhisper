//! KenLM n-gram language model engine for context-aware spell correction.
//!
//! Wraps vendored KenLM C++ (LGPL 2.1+) via a thin FFI layer.
//! Models are pruned trigrams trained on Wikipedia, stored as KenLM binary trie
//! with 8-bit quantization (~50-100 MB per language).
//!
//! Used by `symspell_correct.rs` to score correction candidates in trigram context.

use jona_types::{
    ASREngine, ASRModel, DownloadType, EngineCategory, EngineError, EngineRegistration, GpuMode,
    Language,
};
use serde::Deserialize;
use std::any::Any;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_uint, c_void, CString};
use std::path::Path;

// -- FFI declarations --

extern "C" {
    fn kenlm_load(path: *const c_char) -> *mut c_void;
    fn kenlm_free(model: *mut c_void);
    fn kenlm_state_size() -> c_int;
    fn kenlm_begin_state(model: *mut c_void, state_out: *mut u8);
    fn kenlm_null_state(model: *mut c_void, state_out: *mut u8);
    fn kenlm_score_word(
        model: *mut c_void,
        state_in: *const u8,
        word: *const c_char,
        state_out: *mut u8,
    ) -> f32;
    fn kenlm_vocab_index(model: *mut c_void, word: *const c_char) -> c_uint;
    fn kenlm_order(model: *mut c_void) -> c_int;
    fn kenlm_score_sentence(model: *mut c_void, sentence: *const c_char) -> f32;
}

// -- Safe Rust wrapper --

/// A loaded KenLM binary model (mmap-backed, read-only after load).
pub struct KenLMModel {
    ptr: *mut c_void,
    state_size: usize,
}

// KenLM Model is read-only after loading — safe to share across threads.
unsafe impl Send for KenLMModel {}
unsafe impl Sync for KenLMModel {}

impl KenLMModel {
    /// Load a KenLM binary model from disk.
    pub fn load(path: &Path) -> Result<Self, EngineError> {
        let path_str = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| EngineError::ModelNotFound(path.display().to_string()))?;

        let ptr = unsafe { kenlm_load(path_str.as_ptr()) };
        if ptr.is_null() {
            return Err(EngineError::LaunchFailed(format!(
                "Failed to load KenLM model: {}",
                path.display()
            )));
        }

        let state_size = unsafe { kenlm_state_size() } as usize;
        log::info!(
            "KenLM model loaded: {} (order={}, state_size={})",
            path.display(),
            unsafe { kenlm_order(ptr) },
            state_size
        );

        Ok(Self { ptr, state_size })
    }

    /// Get the begin-of-sentence state.
    pub fn begin_state(&self) -> Vec<u8> {
        let mut state = vec![0u8; self.state_size];
        unsafe { kenlm_begin_state(self.ptr, state.as_mut_ptr()) };
        state
    }

    /// Get the null (empty context) state.
    pub fn null_state(&self) -> Vec<u8> {
        let mut state = vec![0u8; self.state_size];
        unsafe { kenlm_null_state(self.ptr, state.as_mut_ptr()) };
        state
    }

    /// Score a single word in context. Returns (log10_prob, new_state).
    pub fn score_word(&self, state_in: &[u8], word: &str) -> (f32, Vec<u8>) {
        let mut state_out = vec![0u8; self.state_size];
        let c_word = CString::new(word).unwrap_or_default();
        let prob = unsafe {
            kenlm_score_word(
                self.ptr,
                state_in.as_ptr(),
                c_word.as_ptr(),
                state_out.as_mut_ptr(),
            )
        };
        (prob, state_out)
    }

    /// Check if a word is in the vocabulary (not OOV).
    pub fn is_known(&self, word: &str) -> bool {
        let c_word = CString::new(word).unwrap_or_default();
        let idx = unsafe { kenlm_vocab_index(self.ptr, c_word.as_ptr()) };
        idx != 0 // 0 = <unk>
    }

    /// Score an entire sentence. Returns total log10 probability (includes BOS/EOS).
    pub fn score_sentence(&self, sentence: &str) -> f32 {
        let c_sentence = CString::new(sentence).unwrap_or_default();
        unsafe { kenlm_score_sentence(self.ptr, c_sentence.as_ptr()) }
    }

    /// Score a sequence of words from a given state. Returns (total_log10_prob, final_state).
    pub fn score_sequence(&self, state_in: &[u8], words: &[&str]) -> (f32, Vec<u8>) {
        let mut total = 0.0f32;
        let mut state = state_in.to_vec();
        for word in words {
            let (prob, new_state) = self.score_word(&state, word);
            total += prob;
            state = new_state;
        }
        (total, state)
    }

    /// Get model order (e.g. 3 for trigram).
    pub fn order(&self) -> usize {
        unsafe { kenlm_order(self.ptr) as usize }
    }
}

impl Drop for KenLMModel {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { kenlm_free(self.ptr) };
        }
    }
}

// -- Engine catalogue --

/// Manifest embedded at build time.
const EMBEDDED_MANIFEST: &str = include_str!("../manifest.json");

#[derive(Deserialize)]
struct Manifest {
    languages: HashMap<String, ManifestLang>,
}

#[derive(Deserialize)]
struct ManifestLang {
    label: Option<String>,
    filename: String,
    url: String,
    size: u64,
    ram: Option<u64>,
}

fn model_from_manifest(code: &str, lang: &ManifestLang) -> ASRModel {
    let label = lang
        .label
        .clone()
        .unwrap_or_else(|| code.to_uppercase());

    ASRModel {
        id: format!("lm:{code}"),
        engine_id: "lm".into(),
        label,
        filename: lang.filename.clone(),
        url: lang.url.clone(),
        size: lang.size,
        storage_dir: jona_types::engine_storage_dir("lm"),
        download_type: DownloadType::SingleFile,
        download_marker: None,
        recommended_for: Some(vec![code.into()]),
        params: None,
        ram: lang.ram,
        lang_codes: Some(vec![code.into()]),
        runtime: None,
        quantization: Some("8-bit".into()),
        ..Default::default()
    }
}

fn load_manifest() -> Vec<ASRModel> {
    let cached = jona_types::models_dir().join("lm").join("manifest.json");

    let json = if cached.exists() {
        match std::fs::read_to_string(&cached) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to read cached LM manifest: {e}, using embedded");
                EMBEDDED_MANIFEST.to_string()
            }
        }
    } else {
        EMBEDDED_MANIFEST.to_string()
    };

    parse_manifest(&json)
}

fn parse_manifest(json: &str) -> Vec<ASRModel> {
    let manifest: Manifest = match serde_json::from_str(json) {
        Ok(m) => m,
        Err(e) => {
            log::error!("Failed to parse LM manifest: {e}");
            return Vec::new();
        }
    };

    let mut models: Vec<ASRModel> = manifest
        .languages
        .iter()
        .map(|(code, lang)| model_from_manifest(code, lang))
        .collect();

    models.sort_by(|a, b| a.id.cmp(&b.id));
    models
}

pub struct LMEngine;

impl ASREngine for LMEngine {
    fn engine_id(&self) -> &str {
        "lm"
    }
    fn display_name(&self) -> &str {
        "KenLM Language Models"
    }
    fn category(&self) -> EngineCategory {
        EngineCategory::LanguageModel
    }

    fn models(&self) -> Vec<ASRModel> {
        load_manifest()
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![] // Don't pollute ASR language selector
    }

    fn description(&self) -> &str {
        "N-gram language models for context-aware spell correction (KenLM)."
    }

    fn create_context(
        &self,
        _model: &ASRModel,
        _gpu_mode: GpuMode,
    ) -> Result<Box<dyn Any + Send>, EngineError> {
        // Models are loaded on-demand by symspell_correct, not through ContextMap
        Err(EngineError::LaunchFailed(
            "LM models are loaded on-demand by the spellcheck pipeline".into(),
        ))
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(LMEngine) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jona_types::ASREngine;

    #[test]
    fn engine_registers_as_language_model() {
        let engine = LMEngine;
        assert_eq!(engine.engine_id(), "lm");
        assert_eq!(engine.category(), EngineCategory::LanguageModel);
    }

    #[test]
    fn lm_does_not_pollute_language_selector() {
        let engine = LMEngine;
        assert!(engine.supported_languages().is_empty());
    }

    #[test]
    fn manifest_parses() {
        let models = parse_manifest(EMBEDDED_MANIFEST);
        assert!(models.len() >= 2, "Expected at least FR + EN models");
    }

    #[test]
    fn no_duplicate_models() {
        let models = parse_manifest(EMBEDDED_MANIFEST);
        let mut ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }

    #[test]
    fn all_urls_are_secure() {
        let models = parse_manifest(EMBEDDED_MANIFEST);
        for model in &models {
            assert!(
                model.url.starts_with("https://"),
                "Model {} has insecure URL: {}",
                model.id,
                model.url
            );
        }
    }

    #[test]
    fn state_size_is_reasonable() {
        // KenLM state size should be small (typically < 100 bytes)
        let size = unsafe { kenlm_state_size() };
        assert!(size > 0 && size < 1024, "Unexpected state size: {size}");
    }
}
