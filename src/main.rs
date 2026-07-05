pub mod tor_requests;

use crate::tor_requests::tor_request_builder_traits::{HeaderOrBody, Method};
use crate::tor_requests::TorRequests;
use anyhow::Result;
use http_body_util::BodyExt;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Post {
    pub title: String,
    pub body: String,
    pub id: i64,
    pub user_id: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let tor_requests = TorRequests::new_expensive().await?;
    println!("Tor client set up!");

    for _ in 0..10 {
        let response = tor_requests
            .connect("example.com", 80)
            .await?
            .get("/")
            .empty_body()?
            .send()
            .await?;

        println!(
            "{:?}",
            String::from_utf8(
                response
                    .into_response()
                    .into_body()
                    .collect()
                    .await?
                    .to_bytes()
                    .to_vec()
            )?
        );
    }

    Ok(())
}
