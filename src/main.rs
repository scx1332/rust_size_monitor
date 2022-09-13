mod proxy_management;

use chrono::{DateTime, Local};

use std::path::{Path, PathBuf};
use std::{env, fs, io};


use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use flexi_logger::*;


use rusqlite::{Connection, Result};

use structopt::StructOpt;

use ya_http_proxy_client::api::ManagementApi;
use ya_http_proxy_client::web::WebClient;

use crate::proxy_management::{add_user, get_or_create_endpoint};
use ya_http_proxy_model::{Service};

#[derive(Debug)]
struct PathInfo {
    _id: i32,
    _path: String,
}

#[derive(StructOpt, Debug)]
struct Cli {
    /// Path to a custom configuration file
    #[structopt(long, short, default_value = "config.json")]
    pub _config: PathBuf,
    /// Path to write logs to
    #[structopt(long, short)]
    pub log_dir: Option<PathBuf>,
    /// Listen address
    #[structopt(long, short, default_value = "http://127.0.0.1:7777")]
    pub _management_addr: String,
}

fn setup_logging(log_dir: Option<impl AsRef<Path>>) -> anyhow::Result<()> {
    let log_level = env::var("PROXY_LOG").unwrap_or_else(|_| "info".into());
    env::set_var("PROXY_LOG", &log_level);

    let mut logger = Logger::try_with_str(&log_level)?;

    if let Some(log_dir) = log_dir {
        let log_dir = log_dir.as_ref();

        match fs::create_dir_all(log_dir) {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => anyhow::bail!(format!("invalid log path: {}", e)),
        }

        logger = logger
            .log_to_file(FileSpec::default().directory(log_dir))
            .duplicate_to_stderr(Duplicate::All)
            .rotate(
                Criterion::Size(2 * 1024 * 1024),
                Naming::Timestamps,
                Cleanup::KeepLogFiles(7),
            )
    }

    logger
        .format_for_stderr(log_format)
        .format_for_files(log_format)
        .print_message()
        .start()?;

    Ok(())
}

fn log_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    use std::time::{Duration, UNIX_EPOCH};
    const DATE_FORMAT_STR: &str = "%Y-%m-%d %H:%M:%S%.3f %z";

    let timestamp = now.now().unix_timestamp_nanos() as u64;
    let date = UNIX_EPOCH + Duration::from_nanos(timestamp);
    let local_date = DateTime::<Local>::from(date);

    write!(
        w,
        "[{} {:5} {}] {}",
        local_date.format(DATE_FORMAT_STR),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        record.args()
    )
}

async fn list_services_help() -> anyhow::Result<Vec<Service>> {
    let api_url = "http://127.0.0.1:7777".to_string();
    let client = WebClient::new(api_url.to_string()).map_err(anyhow::Error::from)?;
    let api = ManagementApi::new(client);

    api.get_services().await.map_err(anyhow::Error::from)
}

#[get("/list_services")]
async fn list_services() -> HttpResponse {
    match list_services_help().await {
        Ok(services) => {
            let body = serde_json::to_string(&services).unwrap();
            HttpResponse::Ok()
                .content_type("application/json")
                .body(body)
        }
        Err(err) => HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!("Error when listing services {err}!")),
    }
}
/*
#[get("/create_erigon")]
async fn create_erigon() -> HttpResponse {
    match create_erigon_endp(
        "http://127.0.0.1:7777".to_string(),
        "0.0.0.0:12001".to_string(),
    )
    .await
    {
        Ok(services) => {
            let body = "{\"result\":\"success\"}";
            HttpResponse::Ok()
                .content_type("application/json")
                .body(body)
        }
        Err(err) => HttpResponse::NotFound()
            .content_type("plain/text")
            .body(format!("Error when creating service {err}!")),
    }
}*/

#[get("/create/{service_name}/{port}/{user}/{password}")]
async fn create_erigon2(params: web::Path<(String, u16, String, String)>) -> HttpResponse {
    let tuple = params.into_inner();
    let service_name = tuple.0;
    let port = tuple.1;
    let user = tuple.2;
    let password = tuple.3;

    log::info!("Add service user: {user} password: {password}");
    let service =
        match get_or_create_endpoint(&service_name, format!("0.0.0.0:{port}").as_str()).await {
            Ok(service) => service,
            Err(err) => {
                log::error!("Error when creating service {err}");
                return HttpResponse::BadRequest()
                    .content_type("text/html")
                    .body(format!("Error when creating service {err}!"));
            }
        };

    match add_user(service, user, password).await {
        Ok(()) => {
            let body = "{\"result\":\"success\"}";
            HttpResponse::Ok()
                .content_type("application/json")
                .body(body)
        }
        Err(err) => {
            log::error!("Error when creating service {err}");

            HttpResponse::BadRequest()
                .content_type("text/html")
                .body(format!("Error when creating user {err}!"))
        }
    }
}

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    format!("Hello {name}!")
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenv::dotenv();
    let cli: Cli = Cli::from_args();

    setup_logging(cli.log_dir.as_ref())?;

    let conn = Connection::open("size_history.sqlite")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS path_info (
        id   INTEGER PRIMARY KEY,
        path TEXT    NOT NULL
    )",
        (), // empty list of parameters.
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS path_entry (
        path_info INTEGER NOT NULL,
        path_size INTEGER NOT NULL,
        FOREIGN KEY(path_info) REFERENCES path_info(id)
    )",
        (), // empty list of parameters.
    )?;

    let t = "test".to_string();
    conn.execute("INSERT INTO path_info (path) VALUES (?1)", (&t,))?;

    let mut stmt = conn.prepare("SELECT id, path FROM path_info")?;
    let person_iter = stmt.query_map([], |row| {
        Ok(PathInfo {
            _id: row.get(0)?,
            _path: row.get(1)?,
        })
    })?;

    for person in person_iter {
        println!("Found person {:?}", person.unwrap());
    }

    /*
        let api_url = cli.management_addr.clone();
        let client = WebClient::new(api_url.to_string())?;
        let api = ManagementApi::new(client);

        let addresses = Addresses::new(vec!(SocketAddr::from_str("0.0.0.0:11120").unwrap()));
        let from_uri = Uri::from_str("/").unwrap();
        let to_uri = Uri::from_str("http://127.0.0.1/").unwrap();
        let cs = CreateService{
            name: "Erigon".to_string(),
            server_name: vec!["0.0.0.0".to_string()],
            bind_https: None,
            bind_http: Some(addresses),
            cert: None,
            auth: None,
            from: from_uri,
            to: to_uri,
            timeouts: None,
            cpu_threads: None,
            user: None
        };
        api.create_service(&cs).await?;
    */

    let bind_addr = "127.0.0.1";
    let bind_port = 8080;
    log::info!("Starting server: http://{bind_addr}:{bind_port}");
    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(HttpResponse::Ok))
            .service(greet)
            .service(create_erigon2)
            .service(list_services)
    })
    .bind(("127.0.0.1", 8080))
    .map_err(anyhow::Error::from)?
    .run()
    .await
    .map_err(anyhow::Error::from)
}
