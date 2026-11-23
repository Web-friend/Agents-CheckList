use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::process::{Command, Output};
use std::str::FromStr;
use tempfile::TempDir;

fn database() -> (TempDir, String) {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let url = format!(
        "sqlite://{}",
        directory.path().join("integration.sqlite").display()
    );
    (directory, url)
}

fn cli(database_url: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_checklister"))
        .args(args)
        .env("DATABASE_URL", database_url)
        .output()
        .expect("run checklister")
}

fn stdout_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON on stdout")
}

#[test]
fn cli_persists_and_manages_checklists() {
    let (_directory, url) = database();

    let created = stdout_json(&cli(&url, &["checklist", "create", "Release"]));
    assert_eq!(created["success"], true);
    assert_eq!(created["result"]["name"], "Release");
    let checklist_id = created["result"]["id"].as_i64().unwrap();
    let id_arg = checklist_id.to_string();

    let first = stdout_json(&cli(
        &url,
        &[
            "check",
            "add",
            &id_arg,
            "--title",
            "Build",
            "--description",
            "Build the release",
        ],
    ));
    let first_id = first["result"]["id"].as_i64().unwrap();
    assert_eq!(first["result"]["status"], false);

    let second = stdout_json(&cli(
        &url,
        &[
            "check",
            "add",
            &id_arg,
            "--title",
            "Publish",
            "--description",
            "Publish the release",
        ],
    ));
    let second_id = second["result"]["id"].as_i64().unwrap();

    let listed = stdout_json(&cli(&url, &["check", "list", &id_arg]));
    assert_eq!(listed[0]["id"], first_id);
    assert_eq!(listed[0]["order"], 1);
    assert_eq!(listed[1]["id"], second_id);
    assert_eq!(listed[1]["order"], 2);

    stdout_json(&cli(
        &url,
        &["check", "complete", &id_arg, &first_id.to_string()],
    ));
    let undone = stdout_json(&cli(&url, &["check", "undone", &id_arg]));
    assert_eq!(undone.as_array().unwrap().len(), 1);
    assert_eq!(undone[0]["id"], second_id);

    stdout_json(&cli(&url, &["checklist", "delete", &id_arg]));
    let deleted_show = cli(&url, &["checklist", "show", &id_arg]);
    assert!(!deleted_show.status.success());
    let error: Value = serde_json::from_slice(&deleted_show.stderr).unwrap();
    assert_eq!(error["success"], false);
}

#[tokio::test]
async fn migrates_existing_inline_schema_and_preserves_check_order() {
    let (_directory, url) = database();
    let options = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("create legacy database");
    sqlx::query(
        "CREATE TABLE checklists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE checklist_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            checklist_id INTEGER NOT NULL,
            content TEXT NOT NULL,
            is_completed INTEGER NOT NULL DEFAULT 0,
            position INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO checklists (id, title) VALUES (7, 'Legacy')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO checklist_items (id, checklist_id, content, position)
         VALUES (21, 7, 'Second ID, first order', 0), (19, 7, 'First ID, second order', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let output = stdout_json(&cli(&url, &["checklist", "show", "7"]));
    assert_eq!(output["checklist"]["name"], "Legacy");
    assert_eq!(output["checks"][0]["id"], 21);
    assert_eq!(output["checks"][1]["id"], 19);
    assert_eq!(output["checks"][0]["order"], 1);
    assert_eq!(output["checks"][1]["order"], 2);

    let created = stdout_json(&cli(&url, &["checklist", "create", "After migration"]));
    assert_eq!(created["result"]["name"], "After migration");
}
