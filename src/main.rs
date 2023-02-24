use reqwest::header::{self, CONTENT_TYPE};
use serde_json::Value;
use linked_hash_map::LinkedHashMap;

#[tokio::main]
async fn main() {

    let client = reqwest::Client::new();

    let package_name = "htop";
    let package_path = format!("tools/system/{package_name}");
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

    let commits = json.as_array().unwrap();
    
    let mut versions: LinkedHashMap<&str,&str> = LinkedHashMap::new();
    
    for i in 0..commits.len()-1 {
        let message = commits[i].get("commit").unwrap().get("message").unwrap();
        let message_split: Vec<&str> = message.as_str().unwrap().split(" ").collect();

        if *message_split.first().unwrap() == format!("{package_name}:") && *message_split.get(2).unwrap() == "->" {
            let _from_ver: &str = *message_split.get(1).unwrap();
            let _to_ver: &str = *message_split.get(3).unwrap();
            
            // if hashmap is empty create latest version (_to_ver) at commits[0]
            if versions.is_empty() {
                versions.insert(_to_ver, commits[0].get("sha").unwrap().as_str().unwrap());
            }
            
            // _from_ver version's commit sha is one before where its updated to _to_ver
            versions.insert(_from_ver, commits[i+1].get("sha").unwrap().as_str().unwrap());
        }
    }
    println!("{:?}", versions);
}
