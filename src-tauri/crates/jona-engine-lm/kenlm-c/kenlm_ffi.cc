// Thin C FFI wrapper around KenLM for Rust interop.
// Exposes only the query API (load binary model, score words, vocabulary lookup).

#include "lm/model.hh"
#include <cstring>
#include <cstdlib>

using Model = lm::ngram::Model;

extern "C" {

/// Load a KenLM binary model from disk. Returns opaque pointer or NULL on error.
void* kenlm_load(const char* path) {
    try {
        lm::ngram::Config config;
        config.load_method = util::LAZY; // mmap, minimal upfront RAM
        return new Model(path, config);
    } catch (...) {
        return nullptr;
    }
}

/// Free a loaded model.
void kenlm_free(void* model) {
    delete static_cast<Model*>(model);
}

/// Size of the State struct in bytes (for Rust allocation).
int kenlm_state_size() {
    return static_cast<int>(sizeof(lm::ngram::State));
}

/// Write the begin-of-sentence state into state_out.
void kenlm_begin_state(void* model, void* state_out) {
    auto* m = static_cast<Model*>(model);
    *static_cast<lm::ngram::State*>(state_out) = m->BeginSentenceState();
}

/// Write the null (empty context) state into state_out.
void kenlm_null_state(void* model, void* state_out) {
    auto* m = static_cast<Model*>(model);
    *static_cast<lm::ngram::State*>(state_out) = m->NullContextState();
}

/// Score a single word given input state. Returns log10 probability.
/// Writes the new state to state_out.
float kenlm_score_word(void* model, const void* state_in, const char* word, void* state_out) {
    auto* m = static_cast<Model*>(model);
    const auto* in_state = static_cast<const lm::ngram::State*>(state_in);
    auto* out_state = static_cast<lm::ngram::State*>(state_out);

    lm::WordIndex idx = m->GetVocabulary().Index(word);
    return m->FullScore(*in_state, idx, *out_state).prob;
}

/// Look up a word in the vocabulary. Returns word index (0 = OOV/unknown).
unsigned int kenlm_vocab_index(void* model, const char* word) {
    auto* m = static_cast<Model*>(model);
    return m->GetVocabulary().Index(word);
}

/// Get model order (e.g. 3 for trigram).
int kenlm_order(void* model) {
    auto* m = static_cast<Model*>(model);
    return static_cast<int>(m->Order());
}

/// Score an entire sentence (space-separated words). Returns total log10 probability.
/// Includes BOS and EOS tokens.
float kenlm_score_sentence(void* model, const char* sentence) {
    auto* m = static_cast<Model*>(model);
    lm::ngram::State state, out_state;
    state = m->BeginSentenceState();

    float total = 0.0f;
    const char* p = sentence;

    while (*p) {
        // Skip whitespace
        while (*p == ' ') p++;
        if (!*p) break;

        // Find word end
        const char* word_start = p;
        while (*p && *p != ' ') p++;

        // Copy word (null-terminated)
        size_t len = p - word_start;
        char word[256];
        if (len >= sizeof(word)) len = sizeof(word) - 1;
        memcpy(word, word_start, len);
        word[len] = '\0';

        lm::WordIndex idx = m->GetVocabulary().Index(word);
        total += m->FullScore(state, idx, out_state).prob;
        state = out_state;
    }

    // End of sentence
    lm::WordIndex eos = m->GetVocabulary().EndSentence();
    total += m->FullScore(state, eos, out_state).prob;

    return total;
}

} // extern "C"
