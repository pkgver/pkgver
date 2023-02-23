use reqwest::header::{self, CONTENT_TYPE};
use serde_json::Value;

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let client = reqwest::Client::new();

    let package_name = "glib";
    let package_path = format!("development/libraries/{package_name}");
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

    for commit in json.as_array().unwrap() {
        let message = commit.get("commit").unwrap().get("message").unwrap();
        let message_split: Vec<&str> = message.as_str().unwrap().split(" ").collect();

        if *message_split.first().unwrap() != format!("{package_name}:") {
            continue;
        }

        if *message_split.get(2).unwrap() != "->" {
            continue;
        }

        println!("Message: {}", message)
    }
}
