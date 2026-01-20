#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{DateTime, Utc};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use scraper::{Html, Selector};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Manager, State};
use thiserror::Error;

#[derive(Debug, Error)]
enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
}

type Result<T> = std::result::Result<T, AppError>;

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct DocumentSummary {
    id: i64,
    title: String,
    url: Option<String>,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct ArticleSummary {
    id: i64,
    number: String,
    title: String,
    snippet: String,
    document_title: String,
}

#[derive(Debug, Serialize)]
struct ArticleDetail {
    id: i64,
    number: String,
    title: String,
    body: String,
    document_title: String,
}

#[derive(Debug, Serialize)]
struct MemoSummary {
    id: i64,
    title: String,
    snippet: String,
}

#[derive(Debug, Serialize)]
struct MemoDetail {
    id: i64,
    title: String,
    body: String,
    linked_articles: Vec<LinkedArticle>,
}

#[derive(Debug, Serialize)]
struct LinkedArticle {
    id: i64,
    number: String,
    title: String,
}

#[derive(Debug, Serialize)]
struct AssistantSource {
    label: String,
    article_id: Option<i64>,
    memo_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AssistantAnswer {
    answer: String,
    sources: Vec<AssistantSource>,
}


fn open_connection(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(conn)
}

fn apply_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            url TEXT,
            content_hash TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS articles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
            number TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS memos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            body TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS memo_links (
            memo_id INTEGER NOT NULL REFERENCES memos(id) ON DELETE CASCADE,
            article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
            PRIMARY KEY (memo_id, article_id)
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS article_fts USING fts5(
            title,
            body,
            content='articles',
            content_rowid='id'
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS memo_fts USING fts5(
            title,
            body,
            content='memos',
            content_rowid='id'
        );
        CREATE TRIGGER IF NOT EXISTS articles_ai AFTER INSERT ON articles BEGIN
            INSERT INTO article_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
        END;
        CREATE TRIGGER IF NOT EXISTS articles_au AFTER UPDATE ON articles BEGIN
            INSERT INTO article_fts(article_fts, rowid, title, body)
            VALUES('delete', old.id, old.title, old.body);
            INSERT INTO article_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
        END;
        CREATE TRIGGER IF NOT EXISTS articles_ad AFTER DELETE ON articles BEGIN
            INSERT INTO article_fts(article_fts, rowid, title, body)
            VALUES('delete', old.id, old.title, old.body);
        END;
        CREATE TRIGGER IF NOT EXISTS memos_ai AFTER INSERT ON memos BEGIN
            INSERT INTO memo_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
        END;
        CREATE TRIGGER IF NOT EXISTS memos_au AFTER UPDATE ON memos BEGIN
            INSERT INTO memo_fts(memo_fts, rowid, title, body)
            VALUES('delete', old.id, old.title, old.body);
            INSERT INTO memo_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
        END;
        CREATE TRIGGER IF NOT EXISTS memos_ad AFTER DELETE ON memos BEGIN
            INSERT INTO memo_fts(memo_fts, rowid, title, body)
            VALUES('delete', old.id, old.title, old.body);
        END;
        "
    )?;
    Ok(())
}

fn compute_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

async fn fetch_document(client: &reqwest::Client, url: &str) -> Result<(String, String)> {
    let response = client.get(url).send().await?.error_for_status()?;
    let html = response.text().await?;
    let document = Html::parse_document(&html);
    let title_selector = Selector::parse("title").map_err(|err| AppError::Parse(err.to_string()))?;
    let title = document
        .select(&title_selector)
        .next()
        .map(|node| node.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "Документ Majestic".to_string());

    let content_selector = Selector::parse("article, main, .message-body, .content, .bbWrapper")
        .map_err(|err| AppError::Parse(err.to_string()))?;
    let body_text = document
        .select(&content_selector)
        .next()
        .map(|node| node.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| document.root_element().text().collect::<Vec<_>>().join(" "));
    Ok((title, body_text))
}

fn parse_articles(text: &str) -> Vec<(String, String, String)> {
    let article_regex = Regex::new(r"(?m)^\s*Статья\s+(\d+(?:\.\d+)*)\s*[–—-]?\s*(.+)?$")
        .expect("valid regex");
    let mut articles = Vec::new();
    let mut current_number = "".to_string();
    let mut current_title = "".to_string();
    let mut current_body = String::new();

    for line in text.lines() {
        if let Some(caps) = article_regex.captures(line) {
            if !current_number.is_empty() {
                articles.push((current_number.clone(), current_title.clone(), current_body.trim().to_string()));
            }
            current_number = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            current_title = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
            current_body.clear();
        } else if !current_number.is_empty() {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    if !current_number.is_empty() {
        articles.push((current_number, current_title, current_body.trim().to_string()));
    }

    if articles.is_empty() {
        articles.push(("1".to_string(), "Основной текст".to_string(), text.trim().to_string()));
    }

    articles
}

fn replace_document(conn: &Connection, title: &str, url: &str, text: &str, hash: &str) -> Result<()> {
    let now: DateTime<Utc> = Utc::now();
    let tx = conn.transaction()?;
    let existing_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM documents WHERE url = ?1",
            params![url],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(doc_id) = existing_id {
        tx.execute("DELETE FROM articles WHERE document_id = ?1", params![doc_id])?;
        tx.execute(
            "UPDATE documents SET title = ?1, content_hash = ?2, updated_at = ?3 WHERE id = ?4",
            params![title, hash, now.to_rfc3339(), doc_id],
        )?;
    } else {
        tx.execute(
            "INSERT INTO documents (title, url, content_hash, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![title, url, hash, now.to_rfc3339()],
        )?;
    }

    let doc_id: i64 = tx.query_row(
        "SELECT id FROM documents WHERE url = ?1",
        params![url],
        |row| row.get(0),
    )?;

    let parsed = parse_articles(text);
    for (number, article_title, body) in parsed {
        tx.execute(
            "INSERT INTO articles (document_id, number, title, body) VALUES (?1, ?2, ?3, ?4)",
            params![doc_id, number, article_title, body],
        )?;
    }

    tx.commit()?;
    Ok(())
}

async fn update_sources(state: &AppState, sources: &[String]) -> Result<()> {
    if sources.is_empty() {
        return Ok(());
    }

    let conn = open_connection(&state.db_path)?;
    apply_migrations(&conn)?;
    for url in sources {
        let (title, text) = fetch_document(&state.client, url).await?;
        let hash = compute_hash(&text);
        let existing_hash: Option<String> = conn
            .query_row(
                "SELECT content_hash FROM documents WHERE url = ?1",
                params![url],
                |row| row.get(0),
            )
            .optional()?;
        if existing_hash.as_deref() == Some(&hash) {
            continue;
        }
        replace_document(&conn, &title, url, &text, &hash)?;
    }
    Ok(())
}

#[tauri::command]
async fn initialize_app(state: State<'_, Mutex<AppState>>, sources: Vec<String>) -> Result<()> {
    let state = state.lock().expect("state lock");
    if !state.db_path.exists() {
        if let Some(parent) = state.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let conn = open_connection(&state.db_path)?;
    apply_migrations(&conn)?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;
    drop(conn);
    if count == 0 && !sources.is_empty() {
        update_sources(&state, &sources).await?;
    }
    Ok(())
}

#[tauri::command]
async fn update_from_sources(state: State<'_, Mutex<AppState>>, sources: Vec<String>) -> Result<()> {
    let state = state.lock().expect("state lock");
    update_sources(&state, &sources).await
}

#[tauri::command]
fn list_documents(state: State<'_, Mutex<AppState>>) -> Result<Vec<DocumentSummary>> {
    let state = state.lock().expect("state lock");
    let conn = open_connection(&state.db_path)?;
    let mut stmt = conn.prepare(
        "SELECT id, title, url, updated_at FROM documents ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(DocumentSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            url: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })?;
    let mut docs = Vec::new();
    for row in rows {
        docs.push(row?);
    }
    Ok(docs)
}

#[tauri::command]
fn search_articles(state: State<'_, Mutex<AppState>>, query: String) -> Result<Vec<ArticleSummary>> {
    let state = state.lock().expect("state lock");
    let conn = open_connection(&state.db_path)?;
    let mut stmt = conn.prepare(
        "
        SELECT articles.id, articles.number, articles.title,
               snippet(article_fts, 1, '[', ']', '…', 16) as snippet,
               documents.title as document_title
        FROM article_fts
        JOIN articles ON article_fts.rowid = articles.id
        JOIN documents ON articles.document_id = documents.id
        WHERE article_fts MATCH ?1
        ORDER BY bm25(article_fts)
        LIMIT 50
        ",
    )?;
    let rows = stmt.query_map([query], |row| {
        Ok(ArticleSummary {
            id: row.get(0)?,
            number: row.get(1)?,
            title: row.get(2)?,
            snippet: row.get(3)?,
            document_title: row.get(4)?,
        })
    })?;
    let mut articles = Vec::new();
    for row in rows {
        articles.push(row?);
    }
    Ok(articles)
}

#[tauri::command]
fn search_by_number(state: State<'_, Mutex<AppState>>, number: String) -> Result<Vec<ArticleSummary>> {
    let state = state.lock().expect("state lock");
    let conn = open_connection(&state.db_path)?;
    let mut stmt = conn.prepare(
        "
        SELECT articles.id, articles.number, articles.title,
               substr(articles.body, 1, 160) as snippet,
               documents.title as document_title
        FROM articles
        JOIN documents ON articles.document_id = documents.id
        WHERE articles.number LIKE ?1
        ORDER BY articles.number
        LIMIT 50
        ",
    )?;
    let rows = stmt.query_map([format!("%{}%", number)], |row| {
        Ok(ArticleSummary {
            id: row.get(0)?,
            number: row.get(1)?,
            title: row.get(2)?,
            snippet: row.get(3)?,
            document_title: row.get(4)?,
        })
    })?;
    let mut articles = Vec::new();
    for row in rows {
        articles.push(row?);
    }
    Ok(articles)
}

#[tauri::command]
fn get_article(state: State<'_, Mutex<AppState>>, article_id: i64) -> Result<ArticleDetail> {
    let state = state.lock().expect("state lock");
    let conn = open_connection(&state.db_path)?;
    conn.query_row(
        "
        SELECT articles.id, articles.number, articles.title, articles.body, documents.title
        FROM articles
        JOIN documents ON articles.document_id = documents.id
        WHERE articles.id = ?1
        ",
        params![article_id],
        |row| {
            Ok(ArticleDetail {
                id: row.get(0)?,
                number: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                document_title: row.get(4)?,
            })
        },
    )
    .map_err(AppError::from)
}

#[tauri::command]
fn search_memos(state: State<'_, Mutex<AppState>>, query: String) -> Result<Vec<MemoSummary>> {
    let state = state.lock().expect("state lock");
    let conn = open_connection(&state.db_path)?;
    let mut stmt = conn.prepare(
        "
        SELECT memos.id, memos.title,
               snippet(memo_fts, 1, '[', ']', '…', 16) as snippet
        FROM memo_fts
        JOIN memos ON memo_fts.rowid = memos.id
        WHERE memo_fts MATCH ?1
        ORDER BY bm25(memo_fts)
        LIMIT 50
        ",
    )?;
    let rows = stmt.query_map([query], |row| {
        Ok(MemoSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            snippet: row.get(2)?,
        })
    })?;
    let mut memos = Vec::new();
    for row in rows {
        memos.push(row?);
    }
    Ok(memos)
}

#[tauri::command]
fn get_memo(state: State<'_, Mutex<AppState>>, memo_id: i64) -> Result<MemoDetail> {
    let state = state.lock().expect("state lock");
    let conn = open_connection(&state.db_path)?;
    let memo = conn.query_row(
        "SELECT id, title, body FROM memos WHERE id = ?1",
        params![memo_id],
        |row| {
            Ok(MemoDetail {
                id: row.get(0)?,
                title: row.get(1)?,
                body: row.get(2)?,
                linked_articles: Vec::new(),
            })
        },
    )?;

    let mut stmt = conn.prepare(
        "
        SELECT articles.id, articles.number, articles.title
        FROM memo_links
        JOIN articles ON memo_links.article_id = articles.id
        WHERE memo_links.memo_id = ?1
        ",
    )?;
    let rows = stmt.query_map([memo_id], |row| {
        Ok(LinkedArticle {
            id: row.get(0)?,
            number: row.get(1)?,
            title: row.get(2)?,
        })
    })?;
    let mut linked = Vec::new();
    for row in rows {
        linked.push(row?);
    }

    Ok(MemoDetail {
        linked_articles: linked,
        ..memo
    })
}

#[tauri::command]
fn assistant_answer(state: State<'_, Mutex<AppState>>, query: String) -> Result<AssistantAnswer> {
    let state = state.lock().expect("state lock");
    let conn = open_connection(&state.db_path)?;

    let mut sources = Vec::new();
    let mut answer_parts = Vec::new();

    let mut stmt = conn.prepare(
        "
        SELECT articles.id, articles.number, articles.title, substr(articles.body, 1, 200)
        FROM article_fts
        JOIN articles ON article_fts.rowid = articles.id
        WHERE article_fts MATCH ?1
        ORDER BY bm25(article_fts)
        LIMIT 3
        ",
    )?;
    let articles = stmt.query_map([&query], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    for article in articles {
        let (id, number, title, body) = article?;
        answer_parts.push(format!("Статья {number} {title}: {body}..."));
        sources.push(AssistantSource {
            label: format!("Статья {number} {title}"),
            article_id: Some(id),
            memo_id: None,
        });
    }

    let mut stmt = conn.prepare(
        "
        SELECT memos.id, memos.title, substr(memos.body, 1, 200)
        FROM memo_fts
        JOIN memos ON memo_fts.rowid = memos.id
        WHERE memo_fts MATCH ?1
        ORDER BY bm25(memo_fts)
        LIMIT 2
        ",
    )?;
    let memos = stmt.query_map([&query], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for memo in memos {
        let (id, title, body) = memo?;
        answer_parts.push(format!("Памятка «{title}»: {body}..."));
        sources.push(AssistantSource {
            label: format!("Памятка «{title}»"),
            article_id: None,
            memo_id: Some(id),
        });
    }

    let answer = if answer_parts.is_empty() {
        "Локальная база пока не содержит релевантных материалов. Проверьте источники и обновите базу.".to_string()
    } else {
        format!("{}\n\nИсточники перечислены ниже.", answer_parts.join("\n"))
    };

    Ok(AssistantAnswer { answer, sources })
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_dir = app.path_resolver().app_data_dir().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "app dir")
            })?;
            let db_path = app_dir.join("majestic_law.sqlite");
            let client = reqwest::Client::builder().user_agent("MajesticLawAssistant/1.0").build()?;
            app.manage(Mutex::new(AppState { db_path, client }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            initialize_app,
            update_from_sources,
            list_documents,
            search_articles,
            search_by_number,
            get_article,
            search_memos,
            get_memo,
            assistant_answer
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
