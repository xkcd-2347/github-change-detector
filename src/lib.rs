use std::time::Duration;
use tokio::time::Instant;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use reqwest::StatusCode;
use tokio::time::sleep_until;

pub struct ChangeDetector {
    owner: String,
    repo: String,
    token: String,
    client: reqwest::Client,
    next_query: Instant,
    interval: Duration,
    etag: Option<String>,
}

pub enum EventType {
    PushEvent
}

impl EventType {
    fn as_str(&self) -> &str{
        match self {
            Self::PushEvent => "PushEvent"
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Event {
    pub r#type: String,
    payload: serde_json::Value,
}

impl Event {
    pub fn payload<E: DeserializeOwned>(&self) -> Result<E, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }
}


#[derive(Serialize, Deserialize)]
pub struct PushEvent {
    pub push_id: i64,
}

const BASE_URL: &str = "https://api.github.com";

impl ChangeDetector {
    pub fn new(owner: &str, repo: &str, token: &str) -> Self {
        Self {
            owner: owner.into(),
            repo: repo.into(),
            token: token.into(),
            client: reqwest::Client::new(),
            next_query: Instant::now(),
            interval: Duration::from_secs(300),
            etag: None,
        }
    }

    pub async fn wait_events(&mut self, event_type: EventType) -> Result<Vec<Event>, anyhow::Error> {
        let url = format!("{}/repos/{}/{}/events", BASE_URL, self.owner, self.repo);
        self.query_event(&url, event_type).await
    }

    async fn query_event(&mut self, url: &str, event_type: EventType) -> Result<Vec<Event>, anyhow::Error> {
        loop {
            sleep_until(self.next_query).await;
            println!("Querying {}", url);
            println!("Token: {}", self.token);
            let mut request = self.client.get(url)
                .header("Accept", "application/json")
                .header("User-Agent", "github-change-detector")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .header("Authorization", format!("Bearer {}", self.token.trim_end()));

            if let Some(etag) = &self.etag {
                request = request.header("If-None-Match", etag);
            }

            // Default
            self.next_query = Instant::now() + Duration::from_secs(300);

            let response = request.send().await;
            match response {
                Ok(result) => {
                    if let Some(Ok(Ok(interval))) = result.headers().get("x-poll-interval").map(|s| s.to_str().map(|s| s.parse::<u64>())) {
                        println!("Updating query interval to {}", interval);
                        self.interval = Duration::from_secs(interval);
                    }

                    if let Some(Ok(tag)) = result.headers().get("etag").map(|s| s.to_str()) {
                        println!("Updating tag {}", tag);
                        self.etag.replace(tag.to_string());
                    }

                    self.next_query = Instant::now() + self.interval;
                    println!("Waiting {} seconds before next query", self.interval.as_secs());
                    if result.status() == StatusCode::OK {
                        if let Ok(mut body) = result.json::<Vec<Event>>().await {
                            return Ok(body.drain(..)
                                      .filter(|v| v.r#type == event_type.as_str())
                                      .collect());
                        }
                    } else if result.status() == StatusCode::NOT_MODIFIED {
                        println!("No events since last query");
                    }
                }
                Err(e) => {
                    self.next_query = Instant::now() + self.interval;
                    eprintln!("error waiting for event: {:?}", e);
                    println!("Waiting {} seconds before next query", self.interval.as_secs());
                    return Err(e.into())
                }
            }
        }
    }
}
