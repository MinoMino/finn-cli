use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn search_command_can_emit_json_to_stdout() {
    let server = MockServer::start().await;
    let fixture = include_str!("fixtures/search_response.json");

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param("q", "rtx 4080"))
        .and(query_param("category", "0.93"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(fixture, "application/json"),
        )
        .mount(&server)
        .await;

    let mut command = Command::cargo_bin("finn-cli").expect("binary to build");
    command
        .env("FINN_BASE_URL", server.uri())
        .args([
            "search",
            "rtx",
            "4080",
            "--category",
            "0.93",
            "--page",
            "2",
            "--json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_matches\": 78"))
        .stdout(predicate::str::contains("\"marketplace\": \"Torget\""));
}

#[tokio::test]
async fn search_command_can_resolve_category_words() {
    let server = MockServer::start().await;
    let categories = include_str!("fixtures/categories_response.json");
    let fixture = include_str!("fixtures/search_response.json");

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param_is_missing("q"))
        .and(query_param_is_missing("page"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(categories, "application/json"),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param("q", "rtx 4080"))
        .and(query_param("product_category", "2.93.3215.8368"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(fixture, "application/json"),
        )
        .mount(&server)
        .await;

    let mut command = Command::cargo_bin("finn-cli").expect("binary to build");
    command
        .env("FINN_BASE_URL", server.uri())
        .args([
            "search",
            "rtx",
            "4080",
            "--category",
            "Elektronikk og hvitevarer > Data > Datakomponenter",
            "--json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_matches\": 78"));
}

#[tokio::test]
async fn search_command_can_pick_category_interactively() {
    let server = MockServer::start().await;
    let categories = include_str!("fixtures/categories_response.json");
    let fixture = include_str!("fixtures/search_response.json");

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param_is_missing("q"))
        .and(query_param_is_missing("page"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(categories, "application/json"),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param("q", "rtx 4080"))
        .and(query_param("product_category", "2.93.3215.8368"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(fixture, "application/json"),
        )
        .mount(&server)
        .await;

    let mut command = Command::cargo_bin("finn-cli").expect("binary to build");
    command
        .env("FINN_BASE_URL", server.uri())
        .args([
            "search",
            "rtx",
            "4080",
            "--pick-category",
            "--category",
            "datakomponnter",
            "--json",
        ])
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total_matches\": 78"));
}

#[tokio::test]
async fn search_command_can_write_json_to_a_file() {
    let server = MockServer::start().await;
    let fixture = include_str!("fixtures/search_response.json");
    let dir = tempdir().expect("temp dir");
    let output_path = dir.path().join("results.json");

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param("q", "rtx 4080"))
        .and(query_param("category", "0.93"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(fixture, "application/json"),
        )
        .mount(&server)
        .await;

    let mut command = Command::cargo_bin("finn-cli").expect("binary to build");
    command
        .env("FINN_BASE_URL", server.uri())
        .args([
            "search",
            "rtx",
            "4080",
            "--category",
            "0.93",
            "--page",
            "2",
            "--output",
            output_path.to_str().expect("utf-8 path"),
        ])
        .assert()
        .success();

    let written = fs::read_to_string(output_path).expect("file to be written");
    assert!(written.contains("\"items\""));
    assert!(written.contains("\"total_matches\": 78"));
}

#[tokio::test]
async fn item_command_can_emit_json_to_stdout() {
    let server = MockServer::start().await;
    let fixture = include_str!("fixtures/item_page.html");

    Mock::given(method("GET"))
        .and(path("/recommerce/forsale/item/451260160"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html; charset=utf-8")
                .set_body_string(fixture),
        )
        .mount(&server)
        .await;

    let mut command = Command::cargo_bin("finn-cli").expect("binary to build");
    command
        .env("FINN_BASE_URL", server.uri())
        .args(["item", "451260160", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": 451260160"))
        .stdout(predicate::str::contains("\"buy_now\": true"));
}

#[tokio::test]
async fn categories_command_lists_categories() {
    let server = MockServer::start().await;
    let categories = include_str!("fixtures/categories_response.json");

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param_is_missing("q"))
        .and(query_param_is_missing("page"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(categories, "application/json"),
        )
        .mount(&server)
        .await;

    let mut command = Command::cargo_bin("finn-cli").expect("binary to build");
    command
        .env("FINN_BASE_URL", server.uri())
        .args(["categories", "data"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Elektronikk og hvitevarer > Data > Datakomponenter",
        ));
}
