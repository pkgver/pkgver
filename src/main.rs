use reqwest::header::{self, CONTENT_TYPE};
use serde_json::Value;

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let client = reqwest::Client::new();

    let package_path = "development/libraries/glib";
    let page = 1;

    let body = client.get(format!("https://api.github.com/repos/NixOS/nixpkgs/commits?path=pkgs/{package_path}/default.nix&per_page=100&page={page}"))
        .header(CONTENT_TYPE, "application/json")
        .header(header::USER_AGENT, header::HeaderValue::from_str("My User Agent/1.0").unwrap())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    let json: Value = serde_json::from_str(&body).unwrap();

    println!("json = {:?}", json);
}
