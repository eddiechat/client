use crate::adapters::sqlite::{self, DbPool};
use crate::adapters::sqlite::messages::UnprocessedMessage;
use crate::error::EddieError;
use crate::services::logger;

use email_classifier::rules;
use ndarray::Array2;
use ort::inputs;
use ort::value::Tensor;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

/// Convert ort::Error (not Send+Sync) to anyhow::Error via its Display impl.
fn ort_err<R>(e: ort::Error<R>) -> anyhow::Error {
    anyhow::anyhow!("ort: {}", e.message())
}

// ── Output types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    Chat,
    NotChat,
}

impl Label {
    pub fn as_str(&self) -> &'static str {
        match self {
            Label::Chat => "chat",
            Label::NotChat => "not_chat",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Deterministic,
    Model,
}

#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub label: Label,
    pub confidence: f32,
    pub source: Source,
    pub reason: String,
}

// ── ONNX model state ────────────────────────────────────────────────────────

pub struct ClassifierState {
    session: Mutex<ort::session::Session>,
    tokenizer: Tokenizer,
}

impl ClassifierState {
    pub fn load(model_path: &Path, tokenizer_path: &Path) -> Result<Self, anyhow::Error> {
        let session = ort::session::Session::builder()
            .map_err(|e| ort_err(e))?
            .with_intra_threads(2)
            .map_err(|e| ort_err(e))?
            .commit_from_file(model_path)
            .map_err(|e| ort_err(e))?;
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;
        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
        })
    }
}

// ── Constants matching the training notebook ─────────────────────────────────

const MAX_LENGTH: usize = 512;
const HEAD_TOKENS: usize = 128;
const TAIL_TOKENS: usize = 382;
const N_META: usize = 12;

// ── Public entry point ───────────────────────────────────────────────────────

/// Classify all unprocessed messages for an account using the two-step pipeline.
pub fn classify_messages(
    pool: &DbPool,
    account_id: &str,
    classifier: &Arc<ClassifierState>,
) -> Result<usize, EddieError> {
    let messages = sqlite::messages::get_unprocessed_messages(pool, account_id)?;
    if messages.is_empty() {
        return Ok(0);
    }
    logger::debug(&format!(
        "Classifying messages: account_id={}, pending={}",
        account_id,
        messages.len()
    ));

    let mut count = 0;
    for msg in &messages {
        let result = classify_email(msg, classifier);
        sqlite::messages::update_classification(
            pool,
            &msg.id,
            result.label.as_str(),
            false,
        )?;
        count += 1;
    }

    Ok(count)
}

// ── Two-step pipeline ────────────────────────────────────────────────────────

fn classify_email(
    msg: &UnprocessedMessage,
    classifier: &ClassifierState,
) -> ClassificationResult {
    // Step 1: deterministic rules
    let fields = rules::EmailFields {
        from_address: &msg.from_address,
        subject: msg.subject.as_deref().unwrap_or(""),
        body_text: msg.body_text.as_deref().unwrap_or(""),
        body_html: msg.body_html.as_deref().unwrap_or(""),
        in_reply_to: msg.in_reply_to.as_deref(),
        reference_count: rules::parse_json_array_len(&msg.references_ids),
        to_count: rules::parse_json_array_len(&msg.to_addresses),
        cc_count: rules::parse_json_array_len(&msg.cc_addresses),
        imap_folder: &msg.imap_folder,
        gmail_labels: rules::parse_json_string_array(&msg.gmail_labels),
    };

    match rules::classify(&fields) {
        rules::Verdict::Chat { reason } => ClassificationResult {
            label: Label::Chat,
            confidence: 1.0,
            source: Source::Deterministic,
            reason,
        },
        rules::Verdict::NotChat { reason } => ClassificationResult {
            label: Label::NotChat,
            confidence: 1.0,
            source: Source::Deterministic,
            reason,
        },
        rules::Verdict::Ambiguous => {
            match run_model(classifier, msg) {
                Ok(result) => result,
                Err(e) => {
                    logger::warn(&format!("ONNX model inference failed, defaulting to Chat: {}", e));
                    ClassificationResult {
                        label: Label::Chat,
                        confidence: 0.5,
                        source: Source::Model,
                        reason: format!("model_error:{}", e),
                    }
                }
            }
        }
    }
}

// ── ONNX inference ───────────────────────────────────────────────────────────

fn run_model(
    state: &ClassifierState,
    msg: &UnprocessedMessage,
) -> Result<ClassificationResult, anyhow::Error> {
    // Build input text
    let subject = msg.subject.as_deref().unwrap_or("").trim();
    let body = msg.body_text.as_deref().unwrap_or("").trim();
    let text = if subject.is_empty() {
        body.to_string()
    } else {
        format!("Subject: {subject}\n{body}")
    };

    // Tokenize
    let encoding = state
        .tokenizer
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
    let ids_raw: Vec<u32> = encoding.get_ids().to_vec();
    let ids = truncate_head_tail(ids_raw);
    let len = ids.len();

    let input_ids_arr: Array2<i64> =
        Array2::from_shape_vec((1, len), ids.iter().map(|&x| x as i64).collect())?;
    let attention_mask_arr: Array2<i64> = Array2::ones((1, len));
    let metadata_arr: Array2<f32> =
        Array2::from_shape_vec((1, N_META), extract_metadata(msg).to_vec())?;

    let input_ids = Tensor::from_array(input_ids_arr).map_err(|e| ort_err(e))?;
    let attention_mask = Tensor::from_array(attention_mask_arr).map_err(|e| ort_err(e))?;
    let metadata = Tensor::from_array(metadata_arr).map_err(|e| ort_err(e))?;

    // Run inference
    let mut session = state.session.lock()
        .map_err(|e| anyhow::anyhow!("Session lock poisoned: {}", e))?;
    let outputs = session.run(inputs![
        "input_ids" => input_ids,
        "attention_mask" => attention_mask,
        "metadata" => metadata,
    ]).map_err(|e| ort_err(e))?;

    let (_shape, logits_data) = outputs["logits"]
        .try_extract_tensor::<f32>()
        .map_err(|e| ort_err(e))?;

    // Softmax over 2 classes
    let l0 = logits_data[0];
    let l1 = logits_data[1];
    let max = l0.max(l1);
    let e0 = (l0 - max).exp();
    let e1 = (l1 - max).exp();
    let sum = e0 + e1;
    let p_chat = e1 / sum;

    let (label, confidence) = if p_chat >= 0.5 {
        (Label::Chat, p_chat)
    } else {
        (Label::NotChat, 1.0 - p_chat)
    };

    Ok(ClassificationResult {
        label,
        confidence,
        source: Source::Model,
        reason: format!("model:{confidence:.3}"),
    })
}

// ── Head+tail truncation ─────────────────────────────────────────────────────

fn truncate_head_tail(ids: Vec<u32>) -> Vec<u32> {
    if ids.len() <= MAX_LENGTH {
        return ids;
    }
    let cls = ids[0];
    let sep = ids[ids.len() - 1];
    let head = &ids[1..HEAD_TOKENS + 1];
    let tail = &ids[ids.len() - TAIL_TOKENS - 1..ids.len() - 1];
    let mut out = vec![cls];
    out.extend_from_slice(head);
    out.push(sep);
    out.extend_from_slice(tail);
    out.push(sep);
    out.truncate(MAX_LENGTH);
    out
}

// ── Metadata feature extraction ──────────────────────────────────────────────

fn extract_metadata(msg: &UnprocessedMessage) -> [f32; N_META] {
    let to_count = rules::parse_json_array_len(&msg.to_addresses);
    let cc_count = rules::parse_json_array_len(&msg.cc_addresses);
    let bcc_count = rules::parse_json_array_len(&msg.bcc_addresses);
    let ref_count = rules::parse_json_array_len(&msg.references_ids);
    let g_labels = rules::parse_json_string_array(&msg.gmail_labels);

    let from = msg.from_address.to_lowercase();
    let body_text = msg.body_text.as_deref().unwrap_or("");
    let body_html = msg.body_html.as_deref().unwrap_or("");
    let folder = msg.imap_folder.to_lowercase();
    let combined = format!("{body_text} {body_html}").to_lowercase();
    let total_rec = (to_count + cc_count + bcc_count) as f32;

    let local = from.split('@').next().unwrap_or("");
    let domain = from.split('@').nth(1).unwrap_or("");

    let auto_prefixes = [
        "noreply",
        "no-reply",
        "donotreply",
        "mailer",
        "newsletter",
        "notifications",
        "updates",
        "alerts",
        "bounce",
        "bounces",
        "marketing",
        "promotions",
        "deals",
        "offers",
    ];

    let sender_auto = auto_prefixes
        .iter()
        .any(|p| local == *p || local.starts_with(&format!("{p}+")))
        || rules::ESP_DOMAINS
            .iter()
            .any(|e| domain == *e || domain.ends_with(&format!(".{e}")));

    let html_len = body_html.len() as f32;
    let text_len = body_text.len() as f32 + 1.0;
    let html_ratio = (html_len / (text_len * 10.0 + 1.0)).min(1.0);
    let has_unsub =
        combined.contains("unsubscribe") || combined.contains("list-unsubscribe");

    [
        msg.in_reply_to.is_some() as u8 as f32,                                    // is_reply
        (ref_count > 0) as u8 as f32,                                              // has_references
        total_rec.min(20.0),                                                        // recipient_count
        (cc_count > 0) as u8 as f32,                                               // has_cc
        sender_auto as u8 as f32,                                                   // sender_auto
        (html_len > 0.0) as u8 as f32,                                             // has_html
        html_ratio,                                                                 // html_ratio
        has_unsub as u8 as f32,                                                     // has_unsubscribe
        msg.has_attachments as u8 as f32,                                           // has_attachments
        rules::AUTOMATED_FOLDER_PATTERNS.iter().any(|p| folder.contains(p)) as u8 as f32, // folder_automated
        g_labels.iter().any(|l| l == "CATEGORY_PROMOTIONS") as u8 as f32,          // gmail_promo
        g_labels.iter().any(|l| l == "CATEGORY_UPDATES") as u8 as f32,             // gmail_update
    ]
}
