use std::num::NonZeroU32;
use std::path::Path;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;

use jona_engines::llm_prompt::LlmError;

/// Cached LLM context: backend + model, reused across calls.
pub struct LlmContext {
    backend: LlamaBackend,
    model: LlamaModel,
    model_id: String,
}

// LlamaBackend/LlamaModel are Send+Sync
unsafe impl Send for LlmContext {}
unsafe impl Sync for LlmContext {}

impl LlmContext {
    /// Load a GGUF model from disk. GPU offloads all layers on macOS Metal.
    pub fn load(path: &Path, model_id: &str) -> Result<Self, LlmError> {
        let backend = LlamaBackend::init()
            .map_err(|e| LlmError::Inference(format!("Failed to init llama backend: {}", e)))?;

        let model_params = LlamaModelParams::default()
            .with_n_gpu_layers(999);

        let model = LlamaModel::load_from_file(&backend, path, &model_params)
            .map_err(|e| LlmError::Inference(format!("Failed to load LLM model: {}", e)))?;

        log::info!("LLM model loaded: {} ({})", model_id, path.display());

        Ok(Self {
            backend,
            model,
            model_id: model_id.to_string(),
        })
    }
}

impl jona_types::HasModelId for LlmContext {
    fn model_id(&self) -> &str {
        &self.model_id
    }
}

/// Clean up transcribed text using a local LLM.
pub fn cleanup_text(ctx: &LlmContext, text: &str, language: &str, max_tokens: usize) -> Result<String, LlmError> {
    let messages = vec![
        LlamaChatMessage::new("system".to_string(), jona_engines::llm_prompt::system_prompt(language))
            .map_err(|e| LlmError::Inference(format!("Failed to create system message: {}", e)))?,
        LlamaChatMessage::new("user".to_string(), text.to_string())
            .map_err(|e| LlmError::Inference(format!("Failed to create user message: {}", e)))?,
    ];

    let template = ctx.model.chat_template(None)
        .map_err(|e| LlmError::Inference(format!("Failed to get chat template: {}", e)))?;
    let prompt = ctx.model.apply_chat_template(&template, &messages, true)
        .map_err(|e| LlmError::Inference(format!("Failed to apply chat template: {}", e)))?;

    let tokens = ctx.model.str_to_token(&prompt, AddBos::Always)
        .map_err(|e| LlmError::Inference(format!("Tokenization failed: {}", e)))?;

    let n_prompt_tokens = tokens.len();
    let max_gen_tokens = max_tokens;

    let n_threads = jona_engines::ort_session::inference_threads() as i32;

    let ctx_size = (n_prompt_tokens + max_gen_tokens + 64) as u32;
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(ctx_size))
        .with_n_batch(512)
        .with_n_threads(n_threads)
        .with_n_threads_batch(n_threads)
        .with_flash_attention_policy(1);

    let mut llama_ctx = ctx.model.new_context(&ctx.backend, ctx_params)
        .map_err(|e| LlmError::Inference(format!("Failed to create LLM context: {}", e)))?;

    let mut batch = LlamaBatch::new(512, 1);
    let last_index = tokens.len() as i32 - 1;
    for (i, token) in (0_i32..).zip(tokens.into_iter()) {
        batch.add(token, i, &[0], i == last_index)
            .map_err(|e| LlmError::Inference(format!("Batch add failed: {}", e)))?;
    }

    llama_ctx.decode(&mut batch)
        .map_err(|e| LlmError::Inference(format!("Prompt decode failed: {}", e)))?;

    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::top_k(40),
        LlamaSampler::top_p(0.9, 1),
        LlamaSampler::temp(0.1),
        LlamaSampler::dist(42),
    ]);

    let mut n_cur = batch.n_tokens();
    let n_len = (n_prompt_tokens + max_gen_tokens) as i32;
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut output = String::new();

    while n_cur < n_len {
        let token = sampler.sample(&llama_ctx, batch.n_tokens() - 1);
        sampler.accept(token);

        if ctx.model.is_eog_token(token) {
            break;
        }

        let piece = ctx.model.token_to_piece(token, &mut decoder, false, None)
            .map_err(|e| LlmError::Inference(format!("Token decode failed: {}", e)))?;
        output.push_str(&piece);

        batch.clear();
        batch.add(token, n_cur, &[0], true)
            .map_err(|e| LlmError::Inference(format!("Batch add failed: {}", e)))?;

        llama_ctx.decode(&mut batch)
            .map_err(|e| LlmError::Inference(format!("Decode failed: {}", e)))?;

        n_cur += 1;
    }

    jona_engines::llm_prompt::sanitize_output(&output, text.len())
}
