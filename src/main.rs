use chrono::{DateTime, Local};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use actix_web::http::Uri;
use flexi_logger::*;

use rusqlite::{Connection, Result};
use structopt::StructOpt;
use ya_http_proxy_client::api::ManagementApi;
use ya_http_proxy_client::web::{WebClient, DEFAULT_MANAGEMENT_API_URL};
use ya_http_proxy_client::Error;
use ya_http_proxy_model::{CreateService, GlobalStats, Service, User};
use tokio;
use std::str::FromStr;

#[derive(Debug)]
struct PathInfo {
    id: i32,
    path: String,
}

#[derive(StructOpt, Debug)]
struct Cli {
    /// Path to a custom configuration file
    #[structopt(long, short, default_value = "config.json")]
    pub config: PathBuf,
    /// Path to write logs to
    #[structopt(long, short)]
    pub log_dir: Option<PathBuf>,
    /// Listen address
    #[structopt(long, short, default_value = "http://127.0.0.1:7777")]
    pub management_addr: String,
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
            id: row.get(0)?,
            path: row.get(1)?,
        })
    })?;

    for person in person_iter {
        println!("Found person {:?}", person.unwrap());
    }

    let api_url = cli.management_addr.clone();
    let client = WebClient::new(api_url.to_string())?;
    let api = ManagementApi::new(client);
    
    let cs = CreateService{
        name: "Erigon".to_string(),
        server_name: vec![],
        bind_https: None,
        bind_http: None,
        cert: None,
        auth: None,
        from: Uri::default(),
        to: Uri::default(),
        timeouts: None,
        cpu_threads: None,
        user: None
    }; 
    api.create_service(&cs).await?;


    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(HttpResponse::Ok))
            .service(greet)
    })
    .bind(("127.0.0.1", 8080))
    .map_err(anyhow::Error::from)?
    .run()
    .await
    .map_err(anyhow::Error::from)
}
