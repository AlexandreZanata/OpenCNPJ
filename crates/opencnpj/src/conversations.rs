//! Account-owned assistant conversations and their persisted interpretation state.

// HTTP rejections carry their response directly.
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use anyhow::Result;
use axum::extract::{Path, Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use opencnpj_nlu::context::Conversation;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::accounts::{Accounts, Denied, now};
use crate::api::Shared;
use crate::gate;

const MAX_CONVERSATIONS: i64 = 100;
const MAX_TURNS: i64 = 100;
const MAX_REPLY_BYTES: usize = 128 << 10;

#[derive(Serialize)]
struct Summary {
    id: i64,
    title: String,
    updated_at: i64,
    turn_count: i64,
}

impl Summary {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self { id: row.get(0)?, title: row.get(1)?, updated_at: row.get(2)?, turn_count: row.get(3)? })
    }
}

#[derive(Serialize)]
struct Turn {
    question: String,
    reply: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Title {
    title: String,
}

fn denied(status: u16, code: &'static str, message: &'static str) -> anyhow::Error {
    Denied { status, code, message: message.into() }.into()
}

pub(crate) fn changed() -> anyhow::Error {
    denied(409, "conversation_changed", "conversation changed; reopen it before continuing")
}

fn missing() -> anyhow::Error {
    denied(404, "conversation_not_found", "conversation not found")
}

fn validated_title(title: &str) -> Result<&str> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(denied(400, "invalid_title", "conversation title must have between 1 and 120 characters"));
    }
    Ok(title)
}

impl Accounts {
    fn list_conversations(&self, user: i64) -> Result<Vec<Summary>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT id, title, updated_at, turn_count FROM conversations WHERE user_id=?1 ORDER BY updated_at DESC, id DESC")?;
        Ok(stmt.query_map([user], Summary::read)?.collect::<rusqlite::Result<_>>()?)
    }

    fn create_conversation(&self, user: i64, title: &str) -> Result<Summary> {
        let title = validated_title(title)?;
        let updated_at = now();
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "INSERT INTO conversations (user_id, title, updated_at)
             SELECT ?1, ?2, ?3 WHERE (SELECT COUNT(*) FROM conversations WHERE user_id=?1) < ?4
             RETURNING id, title, updated_at, turn_count",
        )?;
        stmt.query_row(params![user, title, updated_at, MAX_CONVERSATIONS], Summary::read)
            .optional()?
            .ok_or_else(|| denied(409, "conversation_limit", "delete a conversation before creating another"))
    }

    fn read_conversation(&self, user: i64, id: i64) -> Result<Vec<Turn>> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        let exists = tx.query_row("SELECT 1 FROM conversations WHERE user_id=?1 AND id=?2", [user, id], |_| Ok(())).optional()?;
        if exists.is_none() {
            return Err(missing());
        }
        let mut stmt = tx.prepare("SELECT question, reply FROM conversation_turns WHERE conversation_id=?1 ORDER BY position")?;
        let rows = stmt.query_map([id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        rows.map(|row| {
            let (question, reply) = row?;
            Ok(Turn { question, reply: serde_json::from_str(&reply)? })
        })
        .collect()
    }

    fn rename_conversation(&self, user: i64, id: i64, title: &str) -> Result<Summary> {
        let title = validated_title(title)?;
        let conn = self.conn();
        conn.query_row(
            "UPDATE conversations SET title=?3 WHERE user_id=?1 AND id=?2 RETURNING id, title, updated_at, turn_count",
            params![user, id, title],
            Summary::read,
        )
        .optional()?
        .ok_or_else(missing)
    }

    fn delete_conversation(&self, user: i64, id: i64) -> Result<()> {
        if self.conn().execute("DELETE FROM conversations WHERE user_id=?1 AND id=?2", [user, id])? == 0 {
            return Err(missing());
        }
        Ok(())
    }

    pub(crate) fn conversation_context(&self, user: i64, id: i64) -> Result<(i64, Conversation)> {
        let (revision, context): (i64, String) = self
            .conn()
            .query_row("SELECT turn_count, context FROM conversations WHERE user_id=?1 AND id=?2", [user, id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?
            .ok_or_else(missing)?;
        if revision >= MAX_TURNS {
            return Err(denied(409, "conversation_full", "start a new conversation to continue"));
        }
        let context = if revision == 0 { Conversation::default() } else { serde_json::from_str(&context)? };
        Ok((revision, context))
    }

    pub(crate) fn append_turn(&self, user: i64, id: i64, revision: i64, question: &str, reply: &Value) -> Result<()> {
        let context = serde_json::to_string(&reply["context"])?;
        let reply = serde_json::to_string(reply)?;
        if reply.len() > MAX_REPLY_BYTES {
            return Err(denied(413, "conversation_reply_too_large", "answer is too large to save in a conversation"));
        }
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = tx.execute(
            "UPDATE conversations SET turn_count=turn_count+1, context=?4, updated_at=?5 WHERE user_id=?1 AND id=?2 AND turn_count=?3",
            params![user, id, revision, context, now()],
        )?;
        if updated == 0 {
            return Err(changed());
        }
        tx.execute("INSERT INTO conversation_turns VALUES (?1, ?2, ?3, ?4)", params![id, revision, question, reply])?;
        tx.commit()?;
        Ok(())
    }
}

async fn owner(st: &Shared, req: &mut Request) -> Result<(Arc<Accounts>, i64), Response> {
    let ip = gate::client_ip(req, st.gate.trust_proxy);
    let Some(accounts) = st.gate.accounts.clone() else { return Err(StatusCode::NOT_FOUND.into_response()) };
    let user = st
        .gate
        .authenticate(req.headers(), ip)
        .await?
        .ok_or_else(|| gate::unauthorized("unauthenticated", "sign in to manage your conversations"))?;
    Ok((accounts, user.id))
}

async fn list(State(st): State<Shared>, mut req: Request) -> Response {
    let (accounts, user) = match owner(&st, &mut req).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    reply(gate::blocking(move || accounts.list_conversations(user)).await, StatusCode::OK)
}

async fn create(State(st): State<Shared>, mut req: Request) -> Response {
    let (accounts, user) = match owner(&st, &mut req).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let title: Title = match crate::auth_api::parse_body(req).await {
        Ok(title) => title,
        Err(error) => return error.into_response(),
    };
    reply(gate::blocking(move || accounts.create_conversation(user, &title.title)).await, StatusCode::CREATED)
}

async fn read(State(st): State<Shared>, Path(id): Path<i64>, mut req: Request) -> Response {
    let (accounts, user) = match owner(&st, &mut req).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    reply(gate::blocking(move || accounts.read_conversation(user, id)).await, StatusCode::OK)
}

async fn rename(State(st): State<Shared>, Path(id): Path<i64>, mut req: Request) -> Response {
    let (accounts, user) = match owner(&st, &mut req).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let title: Title = match crate::auth_api::parse_body(req).await {
        Ok(title) => title,
        Err(error) => return error.into_response(),
    };
    reply(gate::blocking(move || accounts.rename_conversation(user, id, &title.title)).await, StatusCode::OK)
}

async fn delete(State(st): State<Shared>, Path(id): Path<i64>, mut req: Request) -> Response {
    let (accounts, user) = match owner(&st, &mut req).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match gate::blocking(move || accounts.delete_conversation(user, id)).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(response) => response,
    }
}

fn reply<T: Serialize>(result: Result<T, Response>, status: StatusCode) -> Response {
    let mut response = match result {
        Ok(value) => (status, Json(value)).into_response(),
        Err(response) => response,
    };
    response.headers_mut().insert(header::CACHE_CONTROL, "private, no-store".parse().expect("valid cache policy"));
    response
}

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/api/v1/conversations", get(list).post(create))
        .route("/api/v1/conversations/{id}", get(read).patch(rename).delete(delete))
}
