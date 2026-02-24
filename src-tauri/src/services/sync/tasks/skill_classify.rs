use crate::adapters::sqlite::sync::{accounts, skill_classify, skills, settings};
use crate::adapters::sqlite::sync::skill_classify::{ClassifyCandidate, Modifiers};
use crate::adapters::sqlite::DbPool;
use crate::adapters::ollama;
use crate::error::EddieError;
use crate::services::logger;
use crate::services::sync::helpers;

const SYSTEM_PROMPT: &str =
    "You are an email classifier. Given a classification prompt and an email, \
     decide if the email matches. Respond with exactly one word: true or false. \
     Do not explain.";

const BATCH_SIZE: u32 = 10;
const BODY_SNIPPET_LEN: usize = 2000;

struct OllamaConfig {
    url: String,
    model: String,
    temperature: f64,
}

/// Resolve the Ollama URL, model, and temperature for a skill.
/// Returns None if no model is configured (skill cannot run).
fn resolve_ollama_config(pool: &DbPool, skill: &skills::Skill) -> Option<OllamaConfig> {
    let url = settings::get_setting(pool, "ollama_url")
        .ok()
        .flatten()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "http://localhost:11434".to_string());

    let parsed: serde_json::Value = serde_json::from_str(&skill.settings)
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let model = parsed.get("ollamaModel")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            settings::get_setting(pool, "ollama_model")
                .ok()
                .flatten()
                .filter(|m| !m.is_empty())
        })?;

    let temperature = parsed.get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    Some(OllamaConfig { url, model, temperature })
}

/// Run skill classification for all enabled skills across all onboarded accounts.
/// Processes one batch of BATCH_SIZE messages per call.
/// Returns true if any classification work was done.
pub async fn run_skill_classify_all(
    app: &tauri::AppHandle,
    pool: &DbPool,
) -> Result<bool, EddieError> {
    let account_ids = accounts::list_onboarded_account_ids(pool)?;

    for account_id in &account_ids {
        let all_skills = skills::list_skills(pool, account_id)?;
        let enabled_skills: Vec<_> = all_skills.into_iter()
            .filter(|s| s.enabled && !s.prompt.is_empty())
            .collect();

        if enabled_skills.is_empty() {
            continue;
        }

        // Get folders that contain messages for this account
        let folders = skill_classify::get_message_folders(pool, account_id)?;
        if folders.is_empty() {
            continue;
        }

        // Ensure cursors and check revisions for all skills
        for skill in &enabled_skills {
            for folder in &folders {
                skill_classify::ensure_cursor(pool, &skill.id, account_id, folder, &skill.revision_hash)?;

                if let Some(cursor) = skill_classify::get_cursor(pool, &skill.id, account_id, folder)? {
                    if cursor.skill_rev != skill.revision_hash {
                        logger::info(&format!(
                            "Skill '{}' revision changed, resetting classification",
                            skill.name
                        ));
                        skill_classify::reset_skill_cursors(pool, &skill.id, &skill.revision_hash)?;
                        break; // All folders for this skill are now reset
                    }
                }
            }
        }

        // Phase 1: Forward batch (new messages — high priority)
        for skill in &enabled_skills {
            let config = match resolve_ollama_config(pool, skill) {
                Some(c) => c,
                None => continue,
            };
            let modifiers = Modifiers::from_json(&skill.modifiers);

            for folder in &folders {
                let cursor = match skill_classify::get_cursor(pool, &skill.id, account_id, folder)? {
                    Some(c) => c,
                    None => continue,
                };

                let batch = skill_classify::get_forward_batch(
                    pool, account_id, folder,
                    cursor.highest_classified_uid,
                    &modifiers, BATCH_SIZE,
                )?;

                if !batch.is_empty() {
                    let matched = classify_batch(pool, &skill.id, &skill.prompt, &config, &batch).await?;

                    if let Some(max_uid) = batch.iter().map(|c| c.imap_uid).max() {
                        skill_classify::update_highest_classified_uid(
                            pool, &skill.id, account_id, folder, max_uid,
                        )?;
                    }

                    if matched > 0 {
                        helpers::status_emit::emit_conversations_updated(app, account_id, 0);
                    }

                    logger::debug(&format!(
                        "Skill '{}': classified {} forward in {}, {} matched",
                        skill.name, batch.len(), folder, matched
                    ));
                    return Ok(true);
                }
            }
        }

        // Phase 2: Backward batch (historical — low priority)
        for skill in &enabled_skills {
            let config = match resolve_ollama_config(pool, skill) {
                Some(c) => c,
                None => continue,
            };
            let modifiers = Modifiers::from_json(&skill.modifiers);

            for folder in &folders {
                let cursor = match skill_classify::get_cursor(pool, &skill.id, account_id, folder)? {
                    Some(c) => c,
                    None => continue,
                };

                // Initialize backward cursor from the forward cursor if not yet set
                if cursor.lowest_classified_uid == 0 && cursor.highest_classified_uid > 0 {
                    skill_classify::update_lowest_classified_uid(
                        pool, &skill.id, account_id, folder,
                        cursor.highest_classified_uid,
                    )?;
                    return Ok(true);
                }

                if cursor.lowest_classified_uid == 0 {
                    continue; // No forward work done yet, nothing to backfill
                }

                let batch = skill_classify::get_backward_batch(
                    pool, account_id, folder,
                    cursor.lowest_classified_uid,
                    &modifiers, BATCH_SIZE,
                )?;

                if !batch.is_empty() {
                    let matched = classify_batch(pool, &skill.id, &skill.prompt, &config, &batch).await?;

                    if let Some(min_uid) = batch.iter().map(|c| c.imap_uid).min() {
                        skill_classify::update_lowest_classified_uid(
                            pool, &skill.id, account_id, folder, min_uid,
                        )?;
                    }

                    if matched > 0 {
                        helpers::status_emit::emit_conversations_updated(app, account_id, 0);
                    }

                    logger::debug(&format!(
                        "Skill '{}': classified {} backward in {}, {} matched",
                        skill.name, batch.len(), folder, matched
                    ));
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

/// Classify a batch of messages using Ollama. Returns count of matches.
async fn classify_batch(
    pool: &DbPool,
    skill_id: &str,
    prompt: &str,
    config: &OllamaConfig,
    candidates: &[ClassifyCandidate],
) -> Result<usize, EddieError> {
    let mut match_ids: Vec<String> = Vec::new();

    for candidate in candidates {
        let body_snippet: String = candidate.body_text
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(BODY_SNIPPET_LEN)
            .collect();

        let user_prompt = format!(
            "Classification prompt: {}\n\nEmail subject: {}\nEmail body: {}",
            prompt,
            candidate.subject.as_deref().unwrap_or(""),
            body_snippet,
        );

        match ollama::chat_complete(
            &config.url,
            &config.model,
            SYSTEM_PROMPT,
            &user_prompt,
            config.temperature,
        ).await {
            Ok(response) => {
                if response.trim().to_lowercase().contains("true") {
                    match_ids.push(candidate.id.clone());
                }
            }
            Err(e) => {
                logger::warn(&format!(
                    "Ollama error during skill classify: {}. Stopping batch.", e
                ));
                // Persist any matches found so far
                if !match_ids.is_empty() {
                    skill_classify::insert_matches_batch(pool, skill_id, &match_ids)?;
                }
                return Err(e);
            }
        }
    }

    let count = match_ids.len();
    if !match_ids.is_empty() {
        skill_classify::insert_matches_batch(pool, skill_id, &match_ids)?;
    }

    Ok(count)
}
