//! The search tokenizer. `assets/tokenize.js` is the same function for queries in the browser;
//! both are checked against `tests/tokenize.json`.

/// Words too common to search for. `and`, `not` and `or` are Datalog clause names and stay
/// searchable. Must equal `STOPWORDS` in `assets/tokenize.js`.
pub const STOPWORDS: &[&str] = &[
    "a", "an", "are", "as", "at", "be", "but", "by", "can", "do", "does", "for", "from", "has",
    "have", "if", "in", "into", "is", "it", "its", "no", "of", "on", "one", "so", "than", "that",
    "the", "their", "then", "there", "these", "this", "to", "was", "we", "were", "when", "which",
    "while", "who", "will", "with", "you", "your",
];

/// Lower-cases ASCII letters, splits into words on anything but ASCII alphanumerics and `-`, and
/// returns each word's parts (split on `-`) of two or more characters that are not stopwords,
/// followed by the whole hyphenated word when it has two or more parts. Repeats are kept.
pub fn tokenize(s: &str) -> Vec<String> {
    let lower = s.to_ascii_lowercase();
    let mut out = Vec::new();
    for word in lower.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
        let parts: Vec<&str> = word.split('-').filter(|p| !p.is_empty()).collect();
        for p in &parts {
            if p.len() >= 2 && !STOPWORDS.contains(p) {
                out.push(p.to_string());
            }
        }
        if parts.len() >= 2 {
            out.push(parts.join("-"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Case {
        input: String,
        tokens: Vec<String>,
    }

    #[test]
    fn shared_vectors() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../tests/tokenize.json")).unwrap();
        assert!(cases.len() > 10);
        for c in cases {
            assert_eq!(tokenize(&c.input), c.tokens, "input {:?}", c.input);
        }
    }

    #[test]
    fn stopwords_match_the_browser() {
        let js = include_str!("../assets/tokenize.js");
        let start = js.find("STOPWORDS = [").expect("STOPWORDS in tokenize.js") + 13;
        let end = start + js[start..].find(']').unwrap();
        let words: Vec<String> = js[start..end]
            .split(',')
            .map(|w| w.trim().trim_matches('"').to_string())
            .filter(|w| !w.is_empty())
            .collect();
        assert_eq!(words, STOPWORDS);
    }
}
