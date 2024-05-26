use std::error::Error;

use bytes::Bytes;
use headers::Authorization;
use http_body_util::{BodyExt, Empty};
use hyper::{Request, Uri};
use hyper_http_proxy::{Intercept, Proxy, ProxyConnector};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let proxy = {
        let proxy_uri = "http://my-proxy:8080".parse().unwrap();
        let mut proxy = Proxy::new(Intercept::All, proxy_uri);
        proxy.set_authorization(Authorization::basic("John Doe", "Agent1234"));
        let connector = HttpConnector::new();

        #[cfg(not(any(feature = "tls", feature = "rustls-base", feature = "openssl-tls")))]
        let proxy_connector = ProxyConnector::from_proxy_unsecured(connector, proxy);

        #[cfg(any(feature = "tls", feature = "rustls-base", feature = "openssl"))]
        let proxy_connector = ProxyConnector::from_proxy(connector, proxy).unwrap();

        proxy_connector
    };

    // Connecting to http will trigger regular GETs and POSTs.
    // We need to manually append the relevant headers to the request
    let uri: Uri = "http://my-remote-website.com".parse().unwrap();
    let mut req = Request::get(uri.clone())
        .body(Empty::<Bytes>::new())
        .unwrap();

    if let Some(headers) = proxy.http_headers(&uri) {
        req.headers_mut().extend(headers.clone().into_iter());
    }

    let client = Client::builder(TokioExecutor::new()).build(proxy);
    let fut_http = async {
        let res = client.request(req).await?;
        let body = res.into_body().collect().await?.to_bytes();

        Ok::<_, Box<dyn Error>>(String::from_utf8(body.to_vec()).unwrap())
    };

    // Connecting to an https uri is straightforward (uses 'CONNECT' method underneath)
    let uri = "https://my-remote-websitei-secured.com".parse().unwrap();
    let fut_https = async {
        let res = client.get(uri).await?;
        let body = res.into_body().collect().await?.to_bytes();

        Ok::<_, Box<dyn Error>>(String::from_utf8(body.to_vec()).unwrap())
    };

    let (http_res, https_res) = futures::future::join(fut_http, fut_https).await;
    let (_, _) = (http_res?, https_res?);

    Ok(())
}
