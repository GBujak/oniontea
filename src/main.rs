pub mod tor_requests;

use crate::tor_requests::tor_request_builder_traits::{HeaderOrBody, Method};
use crate::tor_requests::TorRequests;
use anyhow::Result;
use http_body_util::BodyExt;

const APP_QUALIFIER: &'static str = "com";
const APP_ORGANIZATION: &'static str = "gbujak";
const APP_NAME: &'static str = "oniontea";

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
