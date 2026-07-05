use crate::tor_requests::tor_request_builder_traits::Method;
use crate::{APP_NAME, APP_ORGANIZATION, APP_QUALIFIER};
use anyhow::Result;
use arti_client::config::TorClientConfigBuilder;
use arti_client::TorClient;
use bytes::BytesMut;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::client::conn::http1;
use hyper::client::conn::http1::SendRequest;
use hyper::header::HOST;
use hyper::http::request::Builder as HyperBuilder;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use serde::de::DeserializeOwned;
use std::sync::Arc;

const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

pub struct TorRequests {
    tor_client: Arc<TorClient<tor_rtcompat::PreferredRuntime>>,
}

impl TorRequests {
    pub async fn new_expensive() -> Result<Self> {
        let app_cache_dir =
            directories::ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME)
                .expect("To have a determinable system config directory")
                .cache_dir()
                .to_owned();

        let arti_state_dir = app_cache_dir.clone().join("arti-client-state");
        let arti_cache_dir = app_cache_dir.clone().join("arti-client-cache");

        Ok(Self {
            tor_client: TorClient::create_bootstrapped(
                TorClientConfigBuilder::from_directories(arti_state_dir, arti_cache_dir).build()?,
            )
            .await?,
        })
    }

    pub async fn connect(&self, host_name: impl AsRef<str>, port: u16) -> Result<impl Method> {
        let host_name = host_name.as_ref();
        let stream = self.tor_client.connect((host_name, port)).await?;
        let io = TokioIo::new(stream);
        let (sender, conn) = http1::handshake::<_, Full<Bytes>>(io).await?;
        tokio::spawn(async move {
            let _ = conn.await;
        });

        Ok(TorRequestBuilder {
            builder: Some(Request::builder().header(HOST, host_name)),
            sender,
        })
    }
}

pub struct TorRequestBuilder {
    builder: Option<HyperBuilder>,
    sender: SendRequest<Full<Bytes>>,
}

impl TorRequestBuilder {
    fn of(self, builder: HyperBuilder) -> Self {
        Self {
            builder: Some(builder),
            sender: self.sender,
        }
    }

    fn take_builder(&mut self) -> HyperBuilder {
        self.builder.take().expect("Builder to not be consumed")
    }
}

pub mod tor_request_builder_traits {
    use crate::tor_requests::{TorRequest, TorRequestBuilder};
    use anyhow::Result;
    use http_body_util::Full;
    use hyper::body::Bytes;
    use hyper::http::HeaderName;
    use serde::Serialize;

    pub trait Method {
        fn get(self, uri: impl AsRef<str>) -> impl HeaderOrBody;
        fn post(self, uri: impl AsRef<str>) -> impl HeaderOrBody;
    }

    pub trait HeaderOrBody {
        fn header(self, header_name: HeaderName, value: &str) -> Self;
        fn body(self, body: &impl Serialize) -> Result<TorRequest>;
        fn empty_body(self) -> Result<TorRequest>;
    }

    impl Method for TorRequestBuilder {
        fn get(mut self, uri: impl AsRef<str>) -> impl HeaderOrBody {
            let builder = self.take_builder();
            self.of(builder.uri(uri.as_ref()).method("GET"))
        }

        fn post(mut self, uri: impl AsRef<str>) -> impl HeaderOrBody {
            let builder = self.take_builder();
            self.of(builder.uri(uri.as_ref()).method("POST"))
        }
    }

    impl HeaderOrBody for TorRequestBuilder {
        fn header(mut self, header_name: HeaderName, value: &str) -> Self {
            let builder = self.take_builder();
            self.of(builder.header(header_name, value))
        }

        fn body(mut self, body: &impl Serialize) -> Result<TorRequest> {
            let builder = self.take_builder();
            let bytes = Bytes::from(serde_json::to_vec(body)?);
            let request = builder.body(Full::new(bytes))?;
            Ok(TorRequest {
                body_request: request,
                sender: self.sender,
            })
        }

        fn empty_body(mut self) -> Result<TorRequest> {
            let builder = self.take_builder();
            let request = builder.body(Full::from(Bytes::new()))?;
            Ok(TorRequest {
                body_request: request,
                sender: self.sender,
            })
        }
    }
}

pub struct TorRequest {
    body_request: Request<Full<Bytes>>,
    sender: SendRequest<Full<Bytes>>,
}

impl TorRequest {
    pub async fn send(self) -> Result<TorResponse> {
        let Self {
            mut sender,
            body_request,
        } = self;
        Ok(TorResponse {
            response: sender.send_request(body_request).await?,
        })
    }
}

pub struct TorResponse {
    response: Response<Incoming>,
}

impl TorResponse {
    pub async fn deserialize_body<T: DeserializeOwned>(self) -> Result<T> {
        let mut body = self.response.into_body();
        let mut buf = BytesMut::new();

        while let Some(frame) = body.frame().await {
            let frame = frame?;

            if let Ok(data) = frame.into_data() {
                if buf.len() + data.len() > MAX_BODY_SIZE {
                    anyhow::bail!("response body too large");
                }

                buf.extend_from_slice(&data);
            }
        }

        println!("buffer is {buf:?}");

        Ok(serde_json::from_slice(&buf)?)
    }

    pub fn into_response(self) -> Response<Incoming> {
        self.response
    }
}
