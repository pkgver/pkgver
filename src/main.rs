use clap::{command, Arg, ArgAction};
use linked_hash_map::LinkedHashMap;
use regex::Regex;
use reqwest::header::{self, CONTENT_TYPE};
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

type Version = String;
type CommitHash = String;

#[tokio::main]
async fn main() {
    let args = command!()
        .arg(Arg::new("package").required(true))
        .arg(
            Arg::new("shell")
                .short('s')
                .long("shell")
                .action(ArgAction::SetTrue)
                .required(false),
        )
        .arg(
            Arg::new("install")
                .short('i')
                .long("install")
                .action(ArgAction::SetTrue)
                .required(false),
        )
        .get_matches();

    let package_name = args.get_one::<String>("package").unwrap();
    let mut versions: LinkedHashMap<Version, CommitHash> = LinkedHashMap::new();
    fetch_versions_from_nixpkgs(&mut versions, package_name).await;

    let version_keys: Vec<String> = versions.keys().cloned().collect::<Vec<String>>();
    assert!(!version_keys.is_empty());

    let chosen_version = fzf_select(version_keys);
    let version_commit = versions.get(&chosen_version).unwrap().replace('\"', "");
    println!("{} {}", chosen_version, version_commit);

    if !args.get_flag("shell") && !args.get_flag("shell") {
        informative_message(&version_commit, &package_name)
    }

    if args.get_flag("shell") {
        spawn_shell_with_package(&version_commit, &package_name);
    }

    if args.get_flag("install") {
        install_package(&version_commit, &package_name);
    }
}

fn informative_message(version_commit: &String, package_name: &str) {
    let message = format!(
"
nix-shell (shell) :
    nix-shell -p {} -I nixpkgs=https://github.com/NixOS/nixpkgs/archive/{}.tar.gz

nix-env (install) :
    nix-env -iA {} -f https://github.com/NixOS/nixpkgs/archive/{}.tar.gz

nix file :
    let
        legacy_pkgs = import (fetchTarball https://github.com/NixOS/nixpkgs/archive/{}.tar.gz) {{ }};
        {} = legacy_pkgs.{};
    in
        ...
",
        package_name, version_commit,
        package_name, version_commit,
        version_commit, package_name, package_name
    );
    println!("{}", message)
}

fn install_package(version_commit: &String, package_name: &str) {
    Command::new("nix-env")
        .args([
            "-iA",
            package_name,
            "-f",
            &format!("https://github.com/NixOS/nixpkgs/archive/{version_commit}.tar.gz"),
        ])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to start install")
        .wait()
        .expect("failed to wait on install");
}

fn spawn_shell_with_package(version_commit: &String, package_name: &str) {
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
        .expect("Couldn't fetch from the nixpkgs github!")
        .text()
        .await
        .expect("Couldn't get the text from the fetch response!");

        let json: Value = serde_json::from_str(&body).unwrap();

        let commits = json
            .as_array()
            .expect("Couldn't convert the fetched commits to JSON!");

        //goes backwards
        for i in 0..commits.len() {
            let message = commits[i].get("commit").unwrap().get("message").unwrap();

            let pattern = Regex::new(r"\d+\.\d+|\d+-\d+").unwrap();
            if !pattern.is_match(message.as_str().unwrap()) {
                continue;
            }

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
        .expect("Couldn't fetch from the nixpkgs github!")
        .text()
        .await
        .expect("Couldn't get the text from the fetch response!");

    let path = all_packages
        .split('\n')
        .find(|l| l.contains(format!(" {package_name} = ").as_str()))
        .unwrap_or_else(|| panic!("Couldn't get path for {package_name}"))
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
