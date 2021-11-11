use actix_web::{
    client,
    http::header::{HeaderMap, HeaderName},
    web::Bytes,
    HttpRequest, HttpResponse,
};

use std::time::Duration;

lazy_static! {
    static ref HEADER_X_FORWARDED_FOR: HeaderName =
        HeaderName::from_lowercase(b"x-forwarded-for").unwrap();
    static ref HOP_BY_HOP_HEADERS: Vec<HeaderName> = vec![
        HeaderName::from_lowercase(b"connection").unwrap(),
        HeaderName::from_lowercase(b"proxy-connection").unwrap(),
        HeaderName::from_lowercase(b"keep-alive").unwrap(),
        HeaderName::from_lowercase(b"proxy-authenticate").unwrap(),
        HeaderName::from_lowercase(b"proxy-authorization").unwrap(),
        HeaderName::from_lowercase(b"te").unwrap(),
        HeaderName::from_lowercase(b"trailer").unwrap(),
        HeaderName::from_lowercase(b"transfer-encoding").unwrap(),
        HeaderName::from_lowercase(b"upgrade").unwrap(),
    ];
    static ref HEADER_TE: HeaderName = HeaderName::from_lowercase(b"te").unwrap();
    static ref HEADER_CONNECTION: HeaderName = HeaderName::from_lowercase(b"connection").unwrap();
}

static DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

pub struct ReverseProxy<'a> {
    forward_url: &'a str,
    timeout: Duration,
}

fn remove_connection_headers(headers: &mut HeaderMap) {
    let mut headers_to_delete: Vec<String> = Vec::new();
    let header_connection = &(*HEADER_CONNECTION);

    if headers.contains_key(header_connection) {
        if let Some(connection_header_value) = headers.get(header_connection) {
            if let Ok(connection_header_value) = connection_header_value.to_str() {
                for h in connection_header_value.split(',').map(|s| s.trim()) {
                    headers_to_delete.push(String::from(h));
                }
            }
        }
    }

    for h in headers_to_delete {
        headers.remove(h);
    }
}

impl<'a> ReverseProxy<'a> {
    pub fn new(forward_url: &'a str) -> ReverseProxy<'a> {
        ReverseProxy {
            forward_url,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn timeout(mut self, duration: Duration) -> ReverseProxy<'a> {
        self.timeout = duration;
        self
    }

    fn x_forwarded_for_value(&self, req: &HttpRequest) -> String {
        let mut result = String::new();

        for (key, value) in req.headers() {
            if key == *HEADER_X_FORWARDED_FOR {
                result.push_str(value.to_str().unwrap());
                break;
            }
        }

        if let Some(peer_addr) = req.peer_addr() {
            if !result.is_empty() {
                result.push_str(", ");
            }

            let client_ip_str = &format!("{}", peer_addr.ip());
            result.push_str(client_ip_str);
        }

        result
    }

    pub async fn forward(
        &self,
        req: HttpRequest,
        body: Bytes,
    ) -> Result<HttpResponse, actix_web::Error> {
        let forward_uri = match req.uri().query() {
            Some(query) => format!("{}{}?{}", self.forward_url, req.uri().path(), query),
            None => format!("{}{}", self.forward_url, req.uri().path()),
        };

        let forward_req = client::Client::new()
            .request_from(forward_uri, req.head())
            //.no_default_headers()
            .set_header_if_none(actix_web::http::header::USER_AGENT, "");

        //forward_req.set_header(&(*HEADER_X_FORWARDED_FOR), self.x_forwarded_for_value(&req));

        //remove_connection_headers(forward_req.headers_mut());
        //let forward_req_headers = forward_req.headers_mut();

        /*for h in HOP_BY_HOP_HEADERS.iter() {
            if forward_req_headers.contains_key(h)
            //        && (headers[h] == "" || (h == *HEADER_TE && headers[h] == "trailers"))
            {
                continue;
            }
            forward_req_headers.remove(h);
        }*/

        match forward_req.timeout(self.timeout).send_body(body).await {
            Ok(mut resp) => {
                let response_body = resp.body().await.unwrap();
                let back_body = actix_web::body::Body::from_message(response_body);
                let back_rsp = HttpResponse::with_body(resp.status(), back_body);

                /*for (key, value) in resp.headers() {
                    if !HOP_BY_HOP_HEADERS.contains(key) {
                        back_rsp.header(key.clone(), value.clone());
                    }
                }*/
                //          let mut back_rsp = back_rsp.message_body(back_body);

                //remove_connection_headers(back_rsp.headers_mut());

                Ok(back_rsp)
            }
            Err(e) => Err(e.into()),
        }
    }
}
