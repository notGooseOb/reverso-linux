use crate::lang;
use curl::easy::{Easy, List};
use serde_json::{json, Value};
use std::time::Duration;

const TRANSLATE_URL: &str = "https://api.reverso.net/translate/v1/translation";
const CONTEXT_URL: &str = "https://context.reverso.net/bst-query-service";
const MAX_OPTIONS: usize = 64;
const MAX_EXAMPLES: usize = 128;

#[derive(Clone, Default)]
pub struct Example {
    /// Plain example text, used for direction and deduplication.
    pub source: String,
    pub target: String,
    /// Pango markup for display; `<em>` runs are converted to bold.
    pub source_markup: String,
    pub target_markup: String,
}

impl Example {
    pub fn plain(source: impl Into<String>, target: impl Into<String>) -> Self {
        let source = source.into();
        let target = target.into();
        Self {
            source_markup: escape_markup(&source),
            target_markup: escape_markup(&target),
            source,
            target,
        }
    }

    fn from_html(source: &str, target: &str) -> Self {
        let (source, source_markup) = parse_rich(source);
        let (target, target_markup) = parse_rich(target);
        Self {
            source,
            target,
            source_markup,
            target_markup,
        }
    }
}

#[derive(Clone, Default)]
pub struct TranslationOption {
    pub translation: String,
    pub frequency: i32,
    pub count: i32,
    pub examples: Vec<Example>,
}

pub struct Response {
    pub input: String,
    pub translations: Vec<String>,
    pub options: Vec<TranslationOption>,
}

pub fn is_short_text(text: &str) -> bool {
    let mut words = 0;
    let mut in_word = false;
    let mut punct = 0;
    for c in text.chars() {
        if c.is_whitespace() {
            in_word = false;
        } else {
            if !in_word {
                words += 1;
            }
            in_word = true;
            if c.is_ascii_punctuation() {
                punct += 1;
            }
        }
    }
    words > 0 && words <= 3 && punct <= 1
}

fn post(url: &str, body: &Value, headers: &[(&str, &str)]) -> Result<String, String> {
    let payload = serde_json::to_vec(body).map_err(|e| e.to_string())?;

    let mut easy = Easy::new();
    easy.url(url).map_err(|e| e.to_string())?;
    easy.post(true).map_err(|e| e.to_string())?;
    easy.post_fields_copy(&payload).map_err(|e| e.to_string())?;
    easy.timeout(Duration::from_secs(15))
        .map_err(|e| e.to_string())?;
    easy.connect_timeout(Duration::from_secs(10))
        .map_err(|e| e.to_string())?;
    easy.follow_location(true).map_err(|e| e.to_string())?;

    let mut list = List::new();
    list.append("Content-Type: application/json")
        .map_err(|e| e.to_string())?;
    for (key, value) in headers {
        list.append(&format!("{key}: {value}"))
            .map_err(|e| e.to_string())?;
    }
    easy.http_headers(list).map_err(|e| e.to_string())?;

    let mut data = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|chunk| {
                data.extend_from_slice(chunk);
                Ok(chunk.len())
            })
            .map_err(|e| e.to_string())?;
        transfer.perform().map_err(|e| e.to_string())?;
    }

    let status = easy.response_code().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&data).into_owned();
    if status >= 400 {
        Err(format!("HTTP {status}"))
    } else {
        Ok(text)
    }
}

fn translate_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Accept", "*/*"),
        ("Accept-Language", "en-US,en;q=0.9"),
        ("Origin", "https://www.reverso.net"),
        ("Referer", "https://www.reverso.net/"),
        ("Connection", "keep-alive"),
        ("Sec-Fetch-Dest", "empty"),
        ("Sec-Fetch-Mode", "cors"),
        ("Sec-Fetch-Site", "same-site"),
        (
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        ),
    ]
}

fn context_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Accept", "application/json"),
        ("Accept-Language", "en-US,en;q=0.9"),
        ("Origin", "https://context.reverso.net"),
        ("Referer", "https://context.reverso.net/"),
        (
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        ),
    ]
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn int_field(v: &Value, key: &str) -> i32 {
    v.get(key).and_then(Value::as_i64).unwrap_or(0) as i32
}

pub fn translate(text: &str, source: &str, target: &str) -> Result<Response, String> {
    let src_code = lang::code(source).ok_or_else(|| format!("unsupported source: {source}"))?;
    let tgt_code = lang::code(target).ok_or_else(|| format!("unsupported target: {target}"))?;

    let body = json!({
        "format": "text",
        "from": src_code,
        "input": text,
        "options": {
            "contextResults": true,
            "origin": "reversomobile",
            "sentenceSplitter": false
        },
        "to": tgt_code
    });

    let raw = post(TRANSLATE_URL, &body, &translate_headers())?;
    let v: Value =
        serde_json::from_str(&raw).map_err(|_| "Failed to parse translate response".to_string())?;

    let mut response = Response {
        input: text.to_string(),
        translations: Vec::new(),
        options: Vec::new(),
    };

    if let Some(arr) = v.get("translation").and_then(Value::as_array) {
        for t in arr {
            if let Some(s) = t.as_str() {
                response.translations.push(s.to_string());
            }
        }
    }

    if let Some(results) = v
        .pointer("/contextResults/results")
        .and_then(Value::as_array)
    {
        for res in results.iter().take(MAX_OPTIONS) {
            let translation = str_field(res, "translation");
            if translation.is_empty() {
                continue;
            }
            let mut opt = TranslationOption {
                translation,
                frequency: int_field(res, "frequency"),
                ..Default::default()
            };

            let sources = res.get("sourceExamples").and_then(Value::as_array);
            let targets = res.get("targetExamples").and_then(Value::as_array);
            if let (Some(sources), Some(targets)) = (sources, targets) {
                opt.count = sources.len() as i32;
                for (s, t) in sources.iter().zip(targets.iter()).take(MAX_EXAMPLES) {
                    let example =
                        Example::from_html(s.as_str().unwrap_or(""), t.as_str().unwrap_or(""));
                    if !example.source.is_empty() || !example.target.is_empty() {
                        opt.examples.push(example);
                    }
                }
            }
            response.options.push(opt);
        }
    }

    // The plain translation endpoint usually returns no dictionary options for
    // single words; ask the context service, like the original app did.
    if response.options.is_empty() && !response.translations.is_empty() {
        if let Ok((options, examples)) =
            context_query(text, source, target, &response.translations[0])
        {
            if !options.is_empty() {
                let mut options = options;
                let fragment = &response.translations[0];
                let idx = options
                    .iter()
                    .position(|o| &o.translation == fragment)
                    .unwrap_or(0);
                options[idx].examples = examples;
                response.options = options;
            }
        }
    }

    if response.options.is_empty() {
        if let Some(first) = response.translations.first().cloned() {
            response.options.push(TranslationOption {
                translation: first,
                ..Default::default()
            });
        }
    }

    Ok(response)
}

pub fn fetch_examples(
    text: &str,
    source: &str,
    target: &str,
    fragment: &str,
) -> Result<Vec<Example>, String> {
    let (_, examples) = context_query(text, source, target, fragment)?;
    Ok(examples)
}

fn context_query(
    text: &str,
    source: &str,
    target: &str,
    fragment: &str,
) -> Result<(Vec<TranslationOption>, Vec<Example>), String> {
    let src = lang::short(source).ok_or("unsupported source")?;
    let tgt = lang::short(target).ok_or("unsupported target")?;

    let body = json!({
        "source_text": text,
        "target_text": fragment,
        "source_lang": src,
        "target_lang": tgt,
        "npage": 1,
        "mode": 0
    });

    let raw = post(CONTEXT_URL, &body, &context_headers())?;
    let v: Value =
        serde_json::from_str(&raw).map_err(|_| "Failed to parse context response".to_string())?;

    let mut options = Vec::new();
    if let Some(entries) = v.get("dictionary_entry_list").and_then(Value::as_array) {
        for e in entries.iter().take(MAX_OPTIONS) {
            let term = str_field(e, "term");
            if term.is_empty() {
                continue;
            }
            options.push(TranslationOption {
                translation: term,
                frequency: int_field(e, "frequency"),
                ..Default::default()
            });
        }
    }

    let mut examples = Vec::new();
    if let Some(list) = v.get("list").and_then(Value::as_array) {
        for item in list.iter().take(MAX_EXAMPLES) {
            let example =
                Example::from_html(&str_field(item, "s_text"), &str_field(item, "t_text"));
            if !example.source.is_empty() || !example.target.is_empty() {
                examples.push(example);
            }
        }
    }

    Ok((options, examples))
}

/// Convert a Reverso HTML fragment into plain text and Pango markup.
///
/// Emphasis tags (`<em>`, `<b>`, `<strong>`) are preserved as bold runs; other
/// markup is dropped and entities are decoded. The plain text is kept for
/// direction detection and deduplication.
fn parse_rich(src: &str) -> (String, String) {
    let mut spans: Vec<(String, bool)> = Vec::new();
    let mut buffer = String::new();
    let mut emphasis = 0usize;
    let mut i = 0;

    while i < src.len() {
        let rest = &src[i..];
        if let Some(stripped) = rest.strip_prefix('<') {
            if let Some(end) = stripped.find('>') {
                let tag = &stripped[..end];
                let closing = tag.starts_with('/');
                let name = tag
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('/')
                    .to_ascii_lowercase();
                if matches!(name.as_str(), "em" | "b" | "strong") {
                    flush_span(&mut spans, &mut buffer, emphasis > 0);
                    if closing {
                        emphasis = emphasis.saturating_sub(1);
                    } else {
                        emphasis += 1;
                    }
                }
                i += end + 2;
                continue;
            }
            // Unterminated tag: treat the remainder as literal text.
            buffer.push_str(&decode_entities(rest));
            break;
        }

        let next = rest.find('<').unwrap_or(rest.len());
        buffer.push_str(&decode_entities(&rest[..next]));
        i += next;
    }
    flush_span(&mut spans, &mut buffer, emphasis > 0);

    let mut plain = String::new();
    let mut markup = String::new();
    for (text, bold) in spans {
        plain.push_str(&text);
        if bold {
            markup.push_str("<b>");
        }
        escape_markup_into(&mut markup, &text);
        if bold {
            markup.push_str("</b>");
        }
    }
    (plain, markup)
}

fn flush_span(spans: &mut Vec<(String, bool)>, buffer: &mut String, bold: bool) {
    if buffer.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut() {
        if last.1 == bold {
            last.0.push_str(buffer);
            buffer.clear();
            return;
        }
    }
    spans.push((std::mem::take(buffer), bold));
}

fn escape_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_markup_into(&mut out, text);
    out
}

fn escape_markup_into(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

fn decode_entities(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let end = rest.find(';').filter(|&i| i <= 10);
        let Some(end) = end else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => {
                entity[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            _ => None,
        };
        if let Some(c) = decoded {
            out.push(c);
            rest = &rest[end + 1..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}
