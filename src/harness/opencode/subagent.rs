//! The linkage from a `subagent` tool call to its child session (product §4.4).
//!
//! A subagent run is a session of its own whose `parent_id` is the session
//! that started it. The parent's `subagent` tool part names it in
//! `state.metadata.sessionID`; when that is missing (or names something that
//! is not a child of the parent), the child whose title is the part's
//! `state.input.description` is taken.

use rusqlite::params;
use serde_json::Value;

use super::db::Db;
use super::load::text_at;
use crate::harness::Result;
use crate::harness::shared::SubagentInfo;
use crate::model::SessionId;

/// A child session of a parent.
struct Child {
    id: String,
    title: Option<String>,
    created: i64,
}

/// The tool part `call` of `parent`.
fn find_part(db: &Db, parent: &str, call: &str) -> Result<Option<Value>> {
    let datas = db.select(
        "SELECT data FROM session_message \
         WHERE session_id = ?1 AND type = 'assistant' AND instr(data, ?2) > 0 \
         ORDER BY seq",
        params![parent, call],
        |row| text_at(row, 0),
    )?;
    for data in datas {
        let Ok(data) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let part = data
            .get("content")
            .and_then(Value::as_array)
            .and_then(|parts| {
                parts.iter().find(|part| {
                    part.get("type").and_then(Value::as_str) == Some("tool")
                        && part.get("id").and_then(Value::as_str) == Some(call)
                })
            });
        if let Some(part) = part {
            return Ok(Some(part.clone()));
        }
    }
    Ok(None)
}

fn children(db: &Db, parent: &str) -> Result<Vec<Child>> {
    db.select(
        "SELECT id, title, time_created FROM session_v2 \
         WHERE parent_id = ?1 ORDER BY time_created, id",
        [parent],
        |row| {
            Ok(Child {
                id: row.get(0)?,
                title: row.get(1)?,
                created: row.get(2)?,
            })
        },
    )
}

fn str_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut at = value;
    for key in path {
        at = at.get(key)?;
    }
    at.as_str().map(str::trim).filter(|s| !s.is_empty())
}

/// The child session started by tool call `call` of `parent`.
pub(super) fn find(db: &Db, parent: &SessionId, call: &str) -> Result<Option<SessionId>> {
    let Some(part) = find_part(db, parent.as_str(), call)? else {
        return Ok(None);
    };
    let children = children(db, parent.as_str())?;
    if let Some(named) = str_at(&part, &["state", "metadata", "sessionID"])
        && let Some(child) = children.iter().find(|c| c.id == named)
    {
        return Ok(Some(SessionId::from(child.id.as_str())));
    }
    // Fallback: the children are in the order they were started. Take the
    // first one with the call's description as its title that started when
    // the call did or later (else the first with that title).
    let Some(description) = str_at(&part, &["state", "input", "description"]) else {
        return Ok(None);
    };
    let started = part
        .get("time")
        .and_then(|t| t.get("created"))
        .and_then(Value::as_i64);
    let matching: Vec<&Child> = children
        .iter()
        .filter(|c| c.title.as_deref().map(str::trim) == Some(description))
        .collect();
    let chosen = matching
        .iter()
        .find(|c| started.is_none_or(|at| c.created >= at))
        .or(matching.first());
    Ok(chosen.map(|c| SessionId::from(c.id.as_str())))
}

/// The agent and the description of the run in child session `id`: from the
/// tool call of its parent that names it, else from the child's own `agent`
/// and `title`. `None` when `id` is not a child session.
pub(super) fn info(db: &Db, id: &SessionId) -> Result<Option<SubagentInfo>> {
    let Some((parent, agent, title)) = db.select_one(
        "SELECT parent_id, agent, title FROM session_v2 WHERE id = ?1 AND parent_id IS NOT NULL",
        [id.as_str()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        },
    )?
    else {
        return Ok(None);
    };
    let mut info = SubagentInfo {
        agent_type: agent.unwrap_or_default(),
        description: title.unwrap_or_default(),
    };
    let datas = db.select(
        "SELECT data FROM session_message \
         WHERE session_id = ?1 AND type = 'assistant' AND instr(data, ?2) > 0 \
         ORDER BY seq",
        params![parent, id.as_str()],
        |row| text_at(row, 0),
    )?;
    'rows: for data in datas {
        let Ok(data) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        for part in data
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if str_at(part, &["state", "metadata", "sessionID"]) == Some(id.as_str()) {
                if let Some(agent) = str_at(part, &["state", "input", "agent"]) {
                    info.agent_type = agent.to_owned();
                }
                if let Some(description) = str_at(part, &["state", "input", "description"]) {
                    info.description = description.to_owned();
                }
                break 'rows;
            }
        }
    }
    Ok(Some(info))
}
