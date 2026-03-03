/*
 * voxtral_tokenizer.h - Tekken tokenizer (decode only)
 *
 * The Tekken tokenizer uses a vocabulary stored in tekken.json.
 * For speech-to-text we only need decoding (token IDs -> text).
 */

#ifndef VOXTRAL_TOKENIZER_H
#define VOXTRAL_TOKENIZER_H

#include <stdint.h>

typedef struct vox_tokenizer vox_tokenizer_t;

/* Load tokenizer from tekken.json */
vox_tokenizer_t *vox_tokenizer_load(const char *path);

/* Free tokenizer */
void vox_tokenizer_free(vox_tokenizer_t *tok);

/* Decode a single token ID to string. Returns pointer to internal storage
 * (valid until tokenizer is freed). Returns NULL for unknown tokens. */
const char *vox_tokenizer_decode(vox_tokenizer_t *tok, int token_id);

/* Decode a sequence of token IDs to string.
 * Returns allocated string (caller must free). */
char *vox_tokenizer_decode_seq(vox_tokenizer_t *tok, const int *tokens, int n_tokens);

/* Get special token IDs */
int vox_tokenizer_bos(vox_tokenizer_t *tok);  /* Beginning of sequence */
int vox_tokenizer_eos(vox_tokenizer_t *tok);  /* End of sequence */

/* Get vocabulary size */
int vox_tokenizer_vocab_size(vox_tokenizer_t *tok);

#endif /* VOXTRAL_TOKENIZER_H */
