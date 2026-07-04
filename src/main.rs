use anyhow::Result;
use arti_client::TorClient;
use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::client::conn::http1;
use hyper::{header, Request};
use hyper_util::rt::TokioIo;

#[tokio::main]
async fn main() -> Result<()> {
    let tor_client = TorClient::create_bootstrapped(Default::default()).await?;
    println!("Tor client set up!");

    for _ in 0..10 {
        let stream = tor_client.connect(("example.com", 80)).await?;

        // Adapt the Tokio IO to Hyper
        let io = TokioIo::new(stream);

        // Create an HTTP/1 connection
        let (mut sender, conn) = http1::handshake(io).await?;

        // Drive the connection in the background
        tokio::spawn(async move {
            let _ = conn.await;
        });

        // Send requests normally
        let req = Request::builder()
            .method("GET")
            .uri("/")
            .header(header::HOST, "example.com")
            .header(header::USER_AGENT, "my-app")
            .body(Empty::<Bytes>::new())?;

        let response = sender.send_request(req).await?;
        println!(
            "{}",
            String::from_utf8(response.into_body().collect().await?.to_bytes().to_vec())?
        );
    }

    Ok(())
}
