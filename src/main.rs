use linked_hash_map::LinkedHashMap;
use reqwest::header::{self, CONTENT_TYPE};
use serde_json::Value;

type Version = String;
type CommitHash = String;

#[tokio::main]
async fn main() {
    let package_name = "glib";

    let mut versions: LinkedHashMap<Version, CommitHash> = LinkedHashMap::new();
    fetch_versions_from_nixpkgs(&mut versions, package_name).await;

    println!("{:#?}", versions);
}

async fn fetch_versions_from_nixpkgs(
    versions: &mut LinkedHashMap<Version, CommitHash>,
    package_name: &str,
) {
    let client = reqwest::Client::new();

    let package_path = format!("development/libraries/{package_name}");
    let mut page = 1;

    loop {
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

        let commits = json.as_array().unwrap();

        //goes backwards
        for i in 0..commits.len() - 1 {
            let message = commits[i].get("commit").unwrap().get("message").unwrap();
            let message_split: Vec<&str> = message.as_str().unwrap().split(' ').collect();

            if commits.len() == 1 {
                versions.insert(
                    message_split.get(3).unwrap().to_string(),
                    commits[i].get("sha").unwrap().to_string(),
                );
                break;
            }

            if *message_split.first().unwrap() == format!("{package_name}:")
                && message_split.len() > 2
                && *message_split.get(2).unwrap() == "->"
            {
                let from_ver = message_split.get(1).unwrap().to_string();
                let to_ver = message_split.get(3).unwrap().to_string();

                // if hashmap is empty create latest version (_to_ver) at commits[0]
                if versions.is_empty() {
                    versions.insert(to_ver, commits[0].get("sha").unwrap().to_string());
                }

                // _from_ver version's commit sha is one before where its updated to _to_ver
                versions.insert(from_ver, commits[i + 1].get("sha").unwrap().to_string());
            }
        }

        // If there are no more pages, then we are done :)
        if commits.len() < 100 {
            break;
        }
        page += 1;
    }
}
