use anyhow::{Context, Result};
use chrono::NaiveDate;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashSet;

pub struct GitHubClient {
    client: Client,
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StarredRepo {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
}

#[derive(Debug, Deserialize)]
struct StarredEdge {
    node: StarredRepo,
    #[serde(rename = "starredAt")]
    starred_at: String,
}

#[derive(Debug, Deserialize)]
struct StarredRepositories {
    #[serde(rename = "edges")]
    edges: Vec<StarredEdge>,
}

#[derive(Debug, Deserialize)]
struct StarredViewer {
    #[serde(rename = "starredRepositories")]
    starred_repositories: StarredRepositories,
}

#[derive(Debug, Deserialize)]
struct StarredReposData {
    viewer: Option<StarredViewer>,
}

#[derive(Debug, Deserialize)]
struct StarredReposResponse {
    data: Option<StarredReposData>,
}

#[derive(Debug, Deserialize)]
struct ForkItem {
    #[serde(rename = "full_name")]
    full_name: String,
    #[serde(rename = "created_at")]
    created_at: String,
    parent: Option<ParentRepo>,
}

#[derive(Debug, Deserialize)]
struct ParentRepo {
    #[serde(rename = "full_name")]
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct ForkSearchResponse {
    items: Vec<ForkItem>,
}

#[derive(Debug, Deserialize)]
struct User {
    login: String,
}

impl GitHubClient {
    pub fn new() -> Result<Self> {
        let token = std::env::var("GITHUB_TOKEN").ok();
        let client = Client::new();
        Ok(Self { client, token })
    }

    fn auth_header(&self) -> Option<String> {
        self.token.as_ref().map(|t| format!("Bearer {}", t))
    }

    pub async fn get_authenticated_user(&self) -> Result<String> {
        let mut req = self
            .client
            .get("https://api.github.com/user")
            .header("User-Agent", "time-tools");

        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", &auth);
        }

        let user: User = req.send().await?.json().await?;
        Ok(user.login)
    }

    pub async fn get_starred_repos(&self, since: &str) -> Result<Vec<(String, String)>> {
        let since_date = NaiveDate::parse_from_str(since, "%Y-%m-%d")?;

        let query = r#"
            query {
                viewer {
                    starredRepositories(first: 100, orderBy: {field: STARRED_AT, direction: DESC}) {
                        edges {
                            node {
                                nameWithOwner
                            }
                            starredAt
                        }
                    }
                }
            }
        "#;

        let mut req = self
            .client
            .post("https://api.github.com/graphql")
            .header("User-Agent", "time-tools");

        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", &auth);
        }

        let resp: StarredReposResponse = req.json(&serde_json::json!({ "query": query })).send().await?.json().await?;

        let mut results = Vec::new();
        if let Some(data) = resp.data {
            if let Some(viewer) = data.viewer {
                for edge in viewer.starred_repositories.edges {
                    let starred_at = edge.starred_at.replace('Z', "+00:00");
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&starred_at) {
                        let date = dt.date_naive();
                        if date >= since_date {
                            results.push((date.format("%Y-%m-%d").to_string(), edge.node.name_with_owner));
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    pub async fn get_forks(&self, owner: &str, since: &str) -> Result<Vec<(String, String, String)>> {
        let since_date = NaiveDate::parse_from_str(since, "%Y-%m-%d")?;

        let mut results = Vec::new();
        let mut page = 1u32;

        loop {
            let url = format!(
                "https://api.github.com/search/repositories?q=fork:true+owner:{}+created:>={}&sort=created&order=desc&per_page=100&page={}",
                owner, since, page
            );

            let mut req = self
                .client
                .get(&url)
                .header("User-Agent", "time-tools");

            if let Some(auth) = self.auth_header() {
                req = req.header("Authorization", &auth);
            }

            let resp: ForkSearchResponse = req.send().await?.json().await?;

            if resp.items.is_empty() {
                break;
            }

            for item in resp.items {
                let created_at = item.created_at.replace('Z', "+00:00");
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&created_at) {
                    let date = dt.date_naive();
                    if date >= since_date {
                        let parent_full_name = item.parent.as_ref()
                            .map(|p| &p.full_name)
                            .unwrap_or(&item.full_name)
                            .clone();
                        results.push((
                            date.format("%Y-%m-%d").to_string(),
                            parent_full_name,
                            item.full_name,
                        ));
                    }
                }
            }

            if page >= 10 {
                break;
            }
            page += 1;
        }

        // Deduplicate by parent repo
        let mut seen = HashSet::new();
        let mut deduped = Vec::new();
        for (date, parent, fork) in results {
            if seen.insert(parent.clone()) {
                deduped.push((date, parent, fork));
            }
        }

        Ok(deduped)
    }
}