use octocrab::Octocrab;
use serde::Deserialize;
use serde::Serialize;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let token = std::env::var("UPDATE_GUTENBERG_PR_GITHUB_TOKEN")
        .expect("UPDATE_GUTENBERG_PR_GITHUB_TOKEN env variable is required");

    let octocrab = Octocrab::builder().personal_token(token).build()?;

    let result: serde_json::Value = octocrab.graphql(graphql_query()).await?;
    let gutenberg_prs =
        Vec::<GutenbergPR>::deserialize(&result["data"]["repository"]["pullRequests"]["nodes"])
            .unwrap();

    process_gutenberg_prs(&octocrab, gutenberg_prs).await?;

    Ok(())
}

async fn process_gutenberg_prs(
    octocrab: &Octocrab,
    gutenberg_prs: Vec<GutenbergPR>,
) -> Result<(), Box<dyn Error>> {
    for gutenberg_pr in gutenberg_prs {
        if gutenberg_pr.head_repository_owner.login != "WordPress" {
            println!(
                "skip {} because owned by {}",
                gutenberg_pr.head_ref_name, gutenberg_pr.head_repository_owner.login
            );
            continue;
        }
        let branch_name = automated_branch_name(gutenberg_pr.number);
        match get_gutenberg_submodule_hash(&octocrab, &branch_name).await {
            Ok(remote_hash) => {
                println!(
                    "head: {}, remote: {}",
                    gutenberg_pr.head_ref_oid, remote_hash
                );
                if gutenberg_pr.head_ref_oid != remote_hash {
                    trigger_gb_mobile_ci(&octocrab, &branch_name, &gutenberg_pr.head_ref_name)
                        .await?
                }
            }
            Err(_) => {
                trigger_gb_mobile_ci(&octocrab, &branch_name, &gutenberg_pr.head_ref_name).await?
            }
        };
    }
    Ok(())
}

fn automated_branch_name(gutenberg_pr_number: u64) -> String {
    format!("automated-gutenberg-update/for-pr-{}", gutenberg_pr_number)
}

async fn get_gutenberg_submodule_hash(
    octocrab: &Octocrab,
    branch_name: &str,
) -> Result<String, Box<dyn Error>> {
    let x = octocrab
        ._get(
            "https://api.github.com/repos/oguzkocer/version-test-bin/contents/gutenberg",
            Some(&[("ref", branch_name)]),
        )
        .await?
        .json::<GithubContent>()
        .await?;
    Ok(x.sha)
}

async fn trigger_gb_mobile_ci(
    octocrab: &Octocrab,
    branch_name: &str,
    gutenberg_branch_name: &str,
) -> Result<(), Box<dyn Error>> {
    println!(
        "Triggering gb_mobile CI for branch_name: {}, gutenberg_hash: {}",
        branch_name, gutenberg_branch_name
    );

    let response = octocrab._post("https://api.github.com/repos/oguzkocer/version-test-bin/actions/workflows/update-gutenberg.yml/dispatches", Some(&GutenbergMobileCIParams::new("trunk", branch_name, gutenberg_branch_name))).await?;

    println!("{:?}", response);

    Ok(())
}

#[derive(Deserialize, Debug)]
struct GithubContent {
    sha: String,
}

fn graphql_query() -> &'static str {
    r#"
{
  repository(owner: "wordpress", name: "gutenberg") {
    pullRequests(first: 100, states: [OPEN], labels: ["Mobile App - i.e. Android or iOS"]) {
      nodes {
        headRefOid
        number
        headRefName
        headRepositoryOwner {
            login
        }
      }
    }
  }
}
"#
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GutenbergPR {
    head_ref_oid: String,
    number: u64,
    head_ref_name: String,
    head_repository_owner: RepositoryOwner,
}

#[derive(Debug, Deserialize)]
struct RepositoryOwner {
    login: String,
}

#[derive(Debug, Serialize)]
struct GutenbergMobileCIInputs {
    pr_branch_name: String,
    gutenberg_branch_name: String,
}

#[derive(Debug, Serialize)]
struct GutenbergMobileCIParams {
    #[serde(rename = "ref")]
    reference: String,
    inputs: GutenbergMobileCIInputs,
}

impl GutenbergMobileCIParams {
    fn new(reference: &str, branch_name: &str, gutenberg_branch_name: &str) -> Self {
        Self {
            reference: reference.to_string(),
            inputs: GutenbergMobileCIInputs {
                pr_branch_name: branch_name.to_string(),
                gutenberg_branch_name: gutenberg_branch_name.to_string(),
            },
        }
    }
}
