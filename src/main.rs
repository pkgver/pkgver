use linked_hash_map::LinkedHashMap;
use reqwest::header::{self, CONTENT_TYPE};
use serde_json::Value;
use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

type Version = String;
type CommitHash = String;

#[tokio::main]
async fn main() {
    let args = env::args().collect::<Vec<String>>();
    let package_name = args.get(1).unwrap();

    let mut versions: LinkedHashMap<Version, CommitHash> = LinkedHashMap::new();
    fetch_versions_from_nixpkgs(&mut versions, package_name).await;

    let version_keys: Vec<String> = versions.keys().cloned().collect::<Vec<String>>();
    let options = fzf_select(version_keys);
    println!("{}", options);
    let version_commit = versions.get(&options).unwrap().replace("\"", "");
    Command::new("nix-shell")
        .args([
            "-p",
            package_name,
            "-I",
            &format!("nixpkgs=https://github.com/NixOS/nixpkgs/archive/{version_commit}.tar.gz"),
        ])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to start shell")
        .wait()
        .expect("failed to wait on shell");
    println!("{:#?}", versions.get(&options).unwrap());
    //println!("{:#?}", versions);
}

async fn fetch_versions_from_nixpkgs(
    versions: &mut LinkedHashMap<Version, CommitHash>,
    package_name: &str,
) {
    let client = reqwest::Client::new();

    let package_path = get_package_path(&client, package_name).await;
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
        for i in 0..commits.len() {
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
                if commits.get(i + 1).is_some() {
                    versions.insert(from_ver, commits[i + 1].get("sha").unwrap().to_string());
                }
            }
        }

        // If there are no more pages, then we are done :)
        if commits.len() < 100 {
            break;
        }
        page += 1;
    }
}

async fn get_package_path(client: &reqwest::Client, package_name: &str) -> String {
    let all_packages = client
        .get("https://raw.githubusercontent.com/NixOS/nixpkgs/master/pkgs/top-level/all-packages.nix")
        .header(CONTENT_TYPE, "application/json")
        .header(
            header::USER_AGENT,
            header::HeaderValue::from_str("My User Agent/1.0").unwrap(),
        )
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    let path = all_packages
        .split('\n')
        .find(|l| l.contains(format!(" {package_name} = ").as_str()))
        .unwrap()
        .split(' ')
        .collect::<Vec<&str>>();

    let path = &path.get(5).unwrap()[3..];

    println!("PATH: {}", path);

    path.to_string()
}

// https://github.com/andrewwillette/rust_fzf
fn fzf_select(fzf_input: Vec<String>) -> String {
    let mut child = Command::new("fzf")
        .arg("--reverse")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn child process");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut fzf_in = String::new();
    for selection in fzf_input {
        fzf_in.push_str(&selection);
        fzf_in.push('\n');
    }
    stdin
        .write_all(fzf_in.as_bytes())
        .expect("Failed to write fzf_input to fzf command stdin");
    let output = child
        .wait_with_output()
        .expect("Failed to read fzf command stdout");
    String::from(std::str::from_utf8(&output.stdout).unwrap().trim())
}
