#include "tree_sitter/parser.h"

enum TokenType {
  BLOCK_COMMENT,
  RAW_STRING,
};

void *tree_sitter_renk_external_scanner_create(void) { return NULL; }
void tree_sitter_renk_external_scanner_destroy(void *payload) {}
unsigned tree_sitter_renk_external_scanner_serialize(void *payload, char *buffer) { return 0; }
void tree_sitter_renk_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {}

static void next(TSLexer *lexer) { lexer->advance(lexer, false); }

// Block comments nest, as in Rust.
static bool scan_block_comment(TSLexer *lexer) {
  next(lexer);
  if (lexer->lookahead != '*') return false;
  next(lexer);
  unsigned depth = 1;
  while (depth > 0 && !lexer->eof(lexer)) {
    if (lexer->lookahead == '/') {
      next(lexer);
      if (lexer->lookahead == '*') {
        next(lexer);
        depth++;
      }
    } else if (lexer->lookahead == '*') {
      next(lexer);
      if (lexer->lookahead == '/') {
        next(lexer);
        depth--;
      }
    } else {
      next(lexer);
    }
  }
  return depth == 0;
}

// `r"..."`, `r##"..."##`, `br"..."` and `cr"..."`: the closing quote needs as many `#` as the opening one.
static bool scan_raw_string(TSLexer *lexer) {
  if (lexer->lookahead == 'b' || lexer->lookahead == 'c') next(lexer);
  if (lexer->lookahead != 'r') return false;
  next(lexer);
  unsigned hashes = 0;
  while (lexer->lookahead == '#') {
    next(lexer);
    hashes++;
  }
  if (lexer->lookahead != '"') return false;
  next(lexer);
  while (!lexer->eof(lexer)) {
    if (lexer->lookahead != '"') {
      next(lexer);
      continue;
    }
    next(lexer);
    unsigned seen = 0;
    while (seen < hashes && lexer->lookahead == '#') {
      next(lexer);
      seen++;
    }
    if (seen == hashes) return true;
  }
  return false;
}

bool tree_sitter_renk_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t' || lexer->lookahead == '\r' || lexer->lookahead == '\n') {
    lexer->advance(lexer, true);
  }
  if (valid_symbols[BLOCK_COMMENT] && lexer->lookahead == '/' && scan_block_comment(lexer)) {
    lexer->result_symbol = BLOCK_COMMENT;
    return true;
  }
  if (valid_symbols[RAW_STRING] && scan_raw_string(lexer)) {
    lexer->result_symbol = RAW_STRING;
    return true;
  }
  return false;
}
