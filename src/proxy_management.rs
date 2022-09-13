use actix_web::http::Uri;
use anyhow::anyhow;
use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use ya_http_proxy_client::api::ManagementApi;
use ya_http_proxy_client::WebClient;
use ya_http_proxy_model::{Addresses, CreateService, CreateUser, Service};

pub fn get_management_api() -> anyhow::Result<ManagementApi> {
    let api_url =
        env::var("YA_HTTP_MANAGEMENT_ADDR").unwrap_or("http://127.0.0.1:7777".to_string());
    let web_client = WebClient::new(api_url).map_err(anyhow::Error::from)?;
    Ok(ManagementApi::new(web_client))
}

pub async fn get_endpoint(service_name: &str) -> anyhow::Result<Service> {
    let api = get_management_api()?;

    api.get_service(service_name)
        .await
        .map_err(anyhow::Error::from)
}

pub async fn add_user(
    service: Service,
    username: String,
    password: String,
) -> anyhow::Result<()> {
    let api = get_management_api()?;

    let cu = CreateUser { username, password };
    api.create_user(&service.inner.name, &cu)
        .await
        .map_err(anyhow::Error::from)?;

    Ok(())
}

pub async fn create_endpoint(service_name: &str, listen_addr: &str) -> anyhow::Result<()> {
    let api = get_management_api()?;
    let listen_addr = SocketAddr::from_str(listen_addr)?;
    let addresses = Addresses::new(vec![listen_addr]);
    let from_uri = Uri::from_str("/")?;
    let to_uri = Uri::from_str("http://127.0.0.1/")?;
    let cs = CreateService {
        name: service_name.to_string(),
        server_name: vec!["127.0.0.1".to_string()],
        bind_https: None,
        bind_http: Some(addresses),
        cert: None,
        auth: None,
        from: from_uri,
        to: to_uri,
        timeouts: None,
        cpu_threads: None,
        user: None,
    };
    api.create_service(&cs).await?;
    Ok(())
}

pub async fn get_or_create_endpoint(
    service_name: &str,
    listen_addr: &str,
) -> anyhow::Result<Service> {
    match get_endpoint(service_name).await {
        Ok(service) => Ok(service),
        Err(err) => {
            //todo: check if really error or just not exists
            log::debug!("Service not found: {err}, creating new one: {service_name}...");
            create_endpoint(service_name, listen_addr).await?;
            log::debug!("Service created... {service_name}");
            get_endpoint(service_name)
                .await
                .map_err(|err| anyhow!("Cannot found service after creation: {err}"))
        }
    }
}
