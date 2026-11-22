/// Character n-gram hashing based embedding.
/// Each word is tokenized into bigrams and trigrams, hashed into a 512-dim vector,
/// then L2-normalized to produce a fixed-size embedding.

pub const EMBEDDING_DIM: usize = 512;

/// Simple FNV-1a hash for a string slice.
fn fnv1a_hash(s: &str) -> u64 {
    let mut hash: u64 = 14695981039346656037u64;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211u64);
    }
    hash
}

/// Tokenize text: lowercase, remove non-alphanumeric (keep spaces), split on whitespace.
fn tokenize(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    cleaned
        .split_whitespace()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Generate character n-grams (bigrams + trigrams) from a word.
fn char_ngrams(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let len = chars.len();
    let mut ngrams = Vec::new();

    // Bigrams
    if len >= 2 {
        for i in 0..len - 1 {
            let gram: String = chars[i..i + 2].iter().collect();
            ngrams.push(gram);
        }
    }

    // Trigrams
    if len >= 3 {
        for i in 0..len - 2 {
            let gram: String = chars[i..i + 3].iter().collect();
            ngrams.push(gram);
        }
    }

    // If word is too short for any ngram, use the word itself
    if ngrams.is_empty() {
        ngrams.push(word.to_string());
    }

    ngrams
}

/// Compute a 512-dim TF n-gram hashing embedding for the given text.
pub fn embed(text: &str) -> Vec<f64> {
    let mut vector = vec![0.0f64; EMBEDDING_DIM];

    let words = tokenize(text);

    if words.is_empty() {
        return vector;
    }

    for word in &words {
        let ngrams = char_ngrams(word);
        for ngram in ngrams {
            let idx = (fnv1a_hash(&ngram) as usize) % EMBEDDING_DIM;
            vector[idx] += 1.0;
        }
    }

    // L2-normalize
    l2_normalize(&mut vector);

    vector
}

/// L2-normalize a vector in-place.
fn l2_normalize(v: &mut Vec<f64>) {
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 1e-10 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Compute cosine similarity between two vectors.
/// Returns 0.0 for zero vectors.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }

    // Both are already L2-normalized from embed(), so dot product IS the cosine similarity,
    // but we recompute for safety in case raw vectors are passed.
    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_nonzero() {
        let v = embed("hello world");
        let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6, "Should be unit vector");
    }

    #[test]
    fn test_cosine_same() {
        let v = embed("what is the capital of France");
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_different() {
        let a = embed("hello world");
        let b = embed("xyz quantum physics");
        let sim = cosine_similarity(&a, &b);
        assert!(sim < 1.0);
    }

    #[test]
    fn test_zero_vector() {
        let a = vec![0.0f64; EMBEDDING_DIM];
        let b = embed("hello");
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }
}
