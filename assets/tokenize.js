// The search tokenizer, applied to queries. It must match `tokenize` in src/tokenize.rs, which
// builds the index; both are checked against tests/tokenize.json.

/** Words too common to search for. Must equal STOPWORDS in src/tokenize.rs. */
export const STOPWORDS = [
  "a", "an", "are", "as", "at", "be", "but", "by", "can", "do", "does", "for", "from", "has",
  "have", "if", "in", "into", "is", "it", "its", "no", "of", "on", "one", "so", "than", "that",
  "the", "their", "then", "there", "these", "this", "to", "was", "we", "were", "when", "which",
  "while", "who", "will", "with", "you", "your",
];

const STOP = new Set(STOPWORDS);

/**
 * Lower-cases ASCII letters, splits into words on anything but ASCII alphanumerics and `-`, and
 * returns each word's parts (split on `-`) of two or more characters that are not stopwords,
 * followed by the whole hyphenated word when it has two or more parts. Repeats are kept.
 */
export function tokenize(s) {
  const lower = s.replace(/[A-Z]/g, (c) => c.toLowerCase());
  const out = [];
  for (const word of lower.split(/[^a-z0-9-]/)) {
    const parts = word.split("-").filter((p) => p !== "");
    for (const p of parts) if (p.length >= 2 && !STOP.has(p)) out.push(p);
    if (parts.length >= 2) out.push(parts.join("-"));
  }
  return out;
}
