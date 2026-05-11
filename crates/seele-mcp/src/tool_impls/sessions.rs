//! Session tools: start, end, summary, capture_passive.

use seele_core::id::SeeleId;
use seele_http::dto::{SaveRequest, SessionEndRequest, SessionStartRequest};
use seele_http::SeeleService;
use serde_json::Value;

use crate::tools::ToolError;

fn parse_id(v: &Value) -> Result<SeeleId, ToolError> {
    let s = v
        .as_str()
        .ok_or_else(|| ToolError::BadParams("id must be a string".into()))?;
    s.parse::<SeeleId>()
        .map_err(|e| ToolError::BadParams(format!("invalid id: {e}")))
}

pub fn start(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let req: SessionStartRequest = serde_json::from_value(params)?;
    let dto = svc.start_session(req)?;
    Ok(serde_json::to_value(dto)?)
}

pub fn end(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let id = parse_id(params.get("id").unwrap_or(&Value::Null))?;
    let summary = params
        .get("summary")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    svc.end_session(id, SessionEndRequest { summary })?;
    Ok(serde_json::json!({"ok": true, "id": id.to_string()}))
}

/// Save a structured summary observation tied to a session. type=memory,
/// topic_key="session/<id>" so subsequent summary writes upsert in-place.
pub fn summary(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let session_id = parse_id(params.get("session_id").unwrap_or(&Value::Null))?;
    let title = params
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("title required".into()))?;
    let summary = params
        .get("summary")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("summary required".into()))?;
    let project = params
        .get("project")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let req = SaveRequest {
        title: title.to_string(),
        content: summary.to_string(),
        r#type: "memory".to_string(),
        project,
        scope: None,
        topic_key: Some(format!("session/{session_id}")),
        session_id: Some(session_id.to_string()),
        tool_name: Some("seele_session_summary".to_string()),
        metadata: Value::Null,
    };
    let resp = svc.save_observation(req)?;
    Ok(serde_json::to_value(resp)?)
}

/// Parse a chat transcript looking for `## Key Learnings:` blocks (or
/// `## Key Learnings`) and save each line item as a `type=learning`
/// observation. Returns the list of saved ids.
pub fn capture_passive(svc: &SeeleService, params: Value) -> Result<Value, ToolError> {
    let transcript = params
        .get("transcript")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadParams("transcript required".into()))?;
    let project = params
        .get("project")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let learnings = extract_key_learnings(transcript);
    if learnings.is_empty() {
        return Ok(serde_json::json!({"saved": [], "count": 0}));
    }
    let mut saved_ids = Vec::with_capacity(learnings.len());
    for item in learnings {
        let req = SaveRequest {
            title: truncate_title(&item),
            content: item,
            r#type: "learning".to_string(),
            project: project.clone(),
            scope: None,
            topic_key: None,
            session_id: session_id.clone(),
            tool_name: Some("seele_capture_passive".to_string()),
            metadata: Value::Null,
        };
        let resp = svc.save_observation(req)?;
        saved_ids.push(resp.id);
    }
    Ok(serde_json::json!({
        "saved": saved_ids,
        "count": saved_ids.len(),
    }))
}

/// Extract bullet items under `## Key Learnings:` (or `## Key Learnings`)
/// up to the next `## ` heading or EOF. Each `- ` or `* ` line is one
/// learning. Multi-line bullets are concatenated into a single item.
fn extract_key_learnings(transcript: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut current: Option<String> = None;
    for raw_line in transcript.lines() {
        let line = raw_line.trim_end();
        if line.starts_with("## ") {
            // Push the buffered bullet before switching sections.
            if let Some(b) = current.take() {
                let trimmed = b.trim().to_string();
                if !trimmed.is_empty() {
                    out.push(trimmed);
                }
            }
            let heading = line.trim_start_matches("## ").trim().to_lowercase();
            in_section = heading == "key learnings" || heading == "key learnings:";
            continue;
        }
        if !in_section {
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            if let Some(b) = current.take() {
                let t = b.trim().to_string();
                if !t.is_empty() {
                    out.push(t);
                }
            }
            current = Some(rest.to_string());
        } else if let Some(buf) = current.as_mut() {
            // Continuation line for the previous bullet.
            if !trimmed.is_empty() {
                buf.push(' ');
                buf.push_str(trimmed);
            }
        }
    }
    if let Some(b) = current.take() {
        let t = b.trim().to_string();
        if !t.is_empty() {
            out.push(t);
        }
    }
    out
}

fn truncate_title(s: &str) -> String {
    const MAX: usize = 120;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        let take: String = s.chars().take(MAX - 1).collect();
        format!("{take}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_learnings_basic() {
        let t = "intro\n## Key Learnings\n- one\n- two\nnot a bullet\n## Next\n- ignored";
        let r = extract_key_learnings(t);
        assert_eq!(r, vec!["one", "two not a bullet"]);
    }

    #[test]
    fn extract_learnings_split_bullets_then_join_continuation() {
        let t = "## Key Learnings\n- first\n  continuation\n- second";
        let r = extract_key_learnings(t);
        assert_eq!(r, vec!["first continuation", "second"]);
    }

    #[test]
    fn extract_learnings_handles_star_bullets() {
        let t = "## Key Learnings:\n* alpha\n* beta\n";
        let r = extract_key_learnings(t);
        assert_eq!(r, vec!["alpha", "beta"]);
    }

    #[test]
    fn extract_learnings_no_section_returns_empty() {
        let t = "no relevant heading here\n- foo\n- bar";
        assert!(extract_key_learnings(t).is_empty());
    }
}
