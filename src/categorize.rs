use bson::{doc, Document};

// `policy_type` is deliberately free text, not a hardcoded enum (PRD 4.1) —
// real extracted values look like "Private Car Liability Only Insurance" or
// "health", never literally "Motor"/"Health". The dashboard's four category
// pills (Life/Health/Motor/Other) need a keyword bucket, not an exact match,
// so this is kept separate from the `policy_type` list filters (which stay
// exact-match — ReportsPage relies on that for its free-text filter box).
const HEALTH_KEYWORDS: &[&str] = &["health", "medical", "mediclaim", "hospital"];
const LIFE_KEYWORDS: &[&str] = &["life", "term", "ulip", "endowment", "pension"];
// "tw" is the standard industry abbreviation for "Two Wheeler" (real extracted
// policy_type values include e.g. "Standalone OD policy for TW", alongside
// ones that spell it out as "...Two Wheeler"/"...Wheeler"). Unlike the other
// keywords here, "tw" is too short to match as a bare substring — that would
// false-positive on unrelated words like "between"/"network" — so it's
// matched as a whole word only; see `SHORT_WORD_KEYWORDS` below.
const MOTOR_KEYWORDS: &[&str] = &["motor", "vehicle", "car", "bike", "wheeler", "auto", "tw"];

// Keywords short/ambiguous enough that a bare substring match would produce
// false positives — matched as a whole word (split on non-alphanumerics)
// instead of `str::contains`/an unanchored regex fragment.
const SHORT_WORD_KEYWORDS: &[&str] = &["tw"];

fn keyword_regex_fragment(keyword: &str) -> String {
    if SHORT_WORD_KEYWORDS.contains(&keyword) {
        format!(r"\b{keyword}\b")
    } else {
        keyword.to_string()
    }
}

fn matches_keyword(lower_text: &str, keyword: &str) -> bool {
    if SHORT_WORD_KEYWORDS.contains(&keyword) {
        lower_text.split(|c: char| !c.is_alphanumeric()).any(|word| word == keyword)
    } else {
        lower_text.contains(keyword)
    }
}

// Returns a Mongo filter fragment for `category` ("life" | "health" | "motor"
// | "other", case-insensitive) matched against a document's `policy_type`
// field, or `None` if `category` isn't one of those four. Merge the result
// into a handler's filter via `visibility::combine_filters`.
pub fn category_filter(category: &str) -> Option<Document> {
    let all_keywords: Vec<&str> = HEALTH_KEYWORDS
        .iter()
        .chain(LIFE_KEYWORDS)
        .chain(MOTOR_KEYWORDS)
        .copied()
        .collect();

    let keywords: &[&str] = match category.to_lowercase().as_str() {
        "health" => HEALTH_KEYWORDS,
        "life" => LIFE_KEYWORDS,
        "motor" => MOTOR_KEYWORDS,
        "other" => {
            let pattern = all_keywords.iter().map(|k| keyword_regex_fragment(k)).collect::<Vec<_>>().join("|");
            return Some(doc! { "policy_type": { "$not": { "$regex": pattern, "$options": "i" } } });
        }
        _ => return None,
    };

    let pattern = keywords.iter().map(|k| keyword_regex_fragment(k)).collect::<Vec<_>>().join("|");
    Some(doc! { "policy_type": { "$regex": pattern, "$options": "i" } })
}

// Same four buckets as `category_filter`, but classifies a single policy_type
// into exactly one of them (for grouping/aggregation) instead of building a
// Mongo filter for one selected bucket. Keyword priority is Health, Life,
// Motor, Other — matches the declaration order above; real policy_type
// values don't straddle these keyword sets in practice.
pub fn classify(policy_type: &str) -> &'static str {
    let lower = policy_type.to_lowercase();
    if HEALTH_KEYWORDS.iter().any(|k| matches_keyword(&lower, k)) {
        "Health"
    } else if LIFE_KEYWORDS.iter().any(|k| matches_keyword(&lower, k)) {
        "Life"
    } else if MOTOR_KEYWORDS.iter().any(|k| matches_keyword(&lower, k)) {
        "Motor"
    } else {
        "Other"
    }
}
