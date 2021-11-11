use std::sync::Mutex;
use std::time::{Duration, Instant};

use actix_web::{route, web, web::Bytes, App, HttpRequest, HttpResponse, HttpServer, Responder};
use log::{error, info};
use redis::{Commands, Connection, RedisResult};

#[macro_use]
extern crate lazy_static;

mod reverse;

macro_rules! measure_time {
    ($name: expr, $expr: expr) => {{
        let now = Instant::now();
        let result = $expr;
        info!(
            "Query '{}' executed in {}ms",
            $name,
            now.elapsed().as_micros() as f64 / 1000.0
        );
        result
    }};
}

struct AppData {
    redis_connection: Mutex<Connection>,
}

#[route(
    "/*",
    method = "CONNECT",
    method = "DELETE",
    method = "GET",
    method = "HEAD",
    method = "OPTIONS",
    method = "PATCH",
    method = "POST",
    method = "PUT",
    method = "TRACE"
)]
async fn index(app_data: web::Data<AppData>, req: HttpRequest, body: Bytes) -> impl Responder {
    let mut redis_connection = app_data.redis_connection.lock().unwrap();

    let headers = req.headers();
    let host_header = headers
        .get("Host")
        .expect("Could not find Host header")
        .to_str()
        .expect("Could not convert to str");

    let now = Instant::now();
    let query_res: RedisResult<i32> = redis_connection.hget("backend", host_header);

    match query_res {
        Ok(port) => {
            let backend_url = format!("http://127.0.0.1:{}", port);

            let res = reverse::ReverseProxy::new(&backend_url)
                .timeout(Duration::from_secs(1))
                .forward(req, body)
                .await
                .unwrap_or_else(|e| e.into());

            info!(
                "Fetched from {} in {}ms",
                backend_url,
                now.elapsed().as_micros() as f64 / 1000.0
            );

            res
        }
        Err(error) => {
            error!("{:?}", error);
            HttpResponse::InternalServerError().body("")
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let redis_url = std::env::var("REDIS_URL").unwrap_or("redis://127.0.0.1/".to_string());

    info!("Connecting to redis backend: {}", redis_url);

    let client = redis::Client::open(redis_url).expect("URL is not correct");
    let mut connection = client
        .get_connection()
        .expect("Unable to open redis connection");

    info!("Connected to redis");

    let _: String = measure_time!(
        "PING",
        redis::cmd("PING")
            .query(&mut connection)
            .expect("Could not ping")
    );

    let app_data = web::Data::new(AppData {
        redis_connection: Mutex::new(connection),
    });

    HttpServer::new(move || App::new().app_data(app_data.clone()).service(index))
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
