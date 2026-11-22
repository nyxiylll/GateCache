use regex::Regex;
use once_cell::sync::Lazy;

#[derive(Debug)]
pub enum SecurityViolation {
    PromptInjection,
    JailbreakAttempt,
    PIIDetected,
    ExcessiveLength,
    SuspiciousPattern,
}

pub struct SecurityCheckResult {
    pub blocked: bool,
    pub violation: Option<SecurityViolation>,
    pub reason: String,
}

impl SecurityCheckResult {
    fn allow() -> Self {
        Self {
            blocked: false,
            violation: None,
            reason: String::new(),
        }
    }

    fn block(violation: SecurityViolation, reason: &str) -> Self {
        Self {
            blocked: true,
            violation: Some(violation),
            reason: reason.to_string(),
        }
    }
}

// ─── Prompt Injection Patterns ───────────────────────────────────────────────

static RE_IGNORE_INSTRUCTIONS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)ignore\s+(all\s+|previous\s+|above\s+|prior\s+)?instructions?").unwrap()
});

static RE_FORGET: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)forget\s+(everything|all|your\s+instructions?|what\s+you\s+(were|was)\s+told)").unwrap()
});

static RE_YOU_ARE_NOW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)you\s+are\s+now\s+(a\s+|an\s+|the\s+)?\w+").unwrap()
});

static RE_ACT_AS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)act\s+as\s+(a\s+|an\s+|the\s+)?\w+").unwrap()
});

static RE_PRETEND: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)pretend\s+(you\s+are|to\s+be|that\s+you('re|\s+are))").unwrap()
});

static RE_DISREGARD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)disregard\s+(your|all|previous)\s+(instructions?|guidelines?|rules?|constraints?|training)").unwrap()
});

static RE_OVERRIDE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)override\s+(your|all|previous)\s+(instructions?|guidelines?|rules?|constraints?)").unwrap()
});

static RE_SYSTEM_PROMPT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(system:\s|\[system\]|<system>)").unwrap()
});

static RE_TEMPLATE_INJECTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\{\{.*?\}\}").unwrap()
});

static RE_SPECIAL_TOKENS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"<\|.*?\|>").unwrap()
});

// ─── Jailbreak Patterns ───────────────────────────────────────────────────────

static RE_JAILBREAK_KEYWORDS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(jailbreak|DAN\s+mode|do\s+anything\s+now|developer\s+mode)").unwrap()
});

static RE_BYPASS_SAFETY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)bypass\s+(the\s+|your\s+)?(safety|filter|restriction|guideline|rule)").unwrap()
});

static RE_FICTIONAL_WORLD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)in\s+(a\s+fictional|an\s+imaginary|a\s+hypothetical)\s+(world|scenario|context|setting|universe)").unwrap()
});

static RE_HYPOTHETICALLY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(hypothetically|theoretically),?\s+(if|suppose|imagine|pretend|assume|let'?s\s+say)").unwrap()
});

// ─── PII Patterns ─────────────────────────────────────────────────────────────

static RE_SSN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap()
});

static RE_CREDIT_CARD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:\d{4}[\s\-]?){3}\d{4}\b").unwrap()
});

static RE_PHONE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:\+\d{1,3}[\s.\-]?)?\(?\d{3}\)?[\s.\-]?\d{3}[\s.\-]?\d{4}\b").unwrap()
});

// ─── Suspicious Code / Injection Patterns ─────────────────────────────────────

static RE_SQL_INJECTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(\bSELECT\b|\bINSERT\b|\bUPDATE\b|\bDELETE\b|\bDROP\b|\bUNION\b)\s+.{0,50}\b(FROM|INTO|TABLE|DATABASE)\b").unwrap()
});

static RE_NULL_BYTE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\x00").unwrap()
});

static RE_SHELL_INJECTION: Lazy<Regex> = Lazy::new(|| {
    // Look for suspicious sequences of shell metacharacters
    Regex::new(r"[;&|]{2,}|`[^`]+`|\$\([^)]+\)|\$\{[^}]+\}").unwrap()
});

/// Get the max prompt length from the environment (cached at first call).
fn max_prompt_length() -> usize {
    static MAX_LEN: once_cell::sync::OnceCell<usize> = once_cell::sync::OnceCell::new();
    *MAX_LEN.get_or_init(|| {
        std::env::var("MAX_PROMPT_LENGTH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10000)
    })
}

/// Run all security checks on the given prompt.
pub fn check_prompt(prompt: &str) -> SecurityCheckResult {
    // 1. Excessive length
    if prompt.len() > max_prompt_length() {
        return SecurityCheckResult::block(
            SecurityViolation::ExcessiveLength,
            &format!("Prompt exceeds maximum allowed length of {} characters", max_prompt_length()),
        );
    }

    // 2. Null bytes
    if RE_NULL_BYTE.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::SuspiciousPattern,
            "Prompt contains null bytes",
        );
    }

    // 3. Prompt injection checks
    if RE_IGNORE_INSTRUCTIONS.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PromptInjection,
            "Prompt injection detected: instruction override attempt",
        );
    }
    if RE_FORGET.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PromptInjection,
            "Prompt injection detected: memory/instruction wipe attempt",
        );
    }
    if RE_YOU_ARE_NOW.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PromptInjection,
            "Prompt injection detected: persona injection",
        );
    }
    if RE_ACT_AS.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PromptInjection,
            "Prompt injection detected: role override (act as)",
        );
    }
    if RE_PRETEND.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PromptInjection,
            "Prompt injection detected: pretend/role-play override",
        );
    }
    if RE_DISREGARD.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PromptInjection,
            "Prompt injection detected: disregard instructions",
        );
    }
    if RE_OVERRIDE.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PromptInjection,
            "Prompt injection detected: override instructions",
        );
    }
    if RE_SYSTEM_PROMPT.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PromptInjection,
            "Prompt injection detected: system prompt injection",
        );
    }
    if RE_TEMPLATE_INJECTION.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PromptInjection,
            "Prompt injection detected: template injection",
        );
    }
    if RE_SPECIAL_TOKENS.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PromptInjection,
            "Prompt injection detected: special token injection",
        );
    }

    // 4. Jailbreak checks
    if RE_JAILBREAK_KEYWORDS.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::JailbreakAttempt,
            "Jailbreak attempt detected: known jailbreak keyword",
        );
    }
    if RE_BYPASS_SAFETY.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::JailbreakAttempt,
            "Jailbreak attempt detected: safety bypass",
        );
    }
    if RE_FICTIONAL_WORLD.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::JailbreakAttempt,
            "Jailbreak attempt detected: fictional world framing",
        );
    }
    if RE_HYPOTHETICALLY.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::JailbreakAttempt,
            "Jailbreak attempt detected: hypothetical framing",
        );
    }

    // 5. PII checks (block SSN, credit card, phone)
    if RE_SSN.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PIIDetected,
            "PII detected: Social Security Number",
        );
    }
    if RE_CREDIT_CARD.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PIIDetected,
            "PII detected: Credit card number",
        );
    }
    if RE_PHONE.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::PIIDetected,
            "PII detected: Phone number",
        );
    }

    // 6. Suspicious code injection
    if RE_SQL_INJECTION.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::SuspiciousPattern,
            "Suspicious pattern detected: SQL injection",
        );
    }
    if RE_SHELL_INJECTION.is_match(prompt) {
        return SecurityCheckResult::block(
            SecurityViolation::SuspiciousPattern,
            "Suspicious pattern detected: shell injection",
        );
    }

    SecurityCheckResult::allow()
}
