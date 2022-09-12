use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use actix_web::http::Uri;
use anyhow::anyhow;
use ya_http_proxy_client::api::ManagementApi;
use ya_http_proxy_client::WebClient;
use ya_http_proxy_model::{Addresses, CreateService, CreateUser, Service};

pub fn get_management_api() -> anyhow::Result<ManagementApi> {
    let api_url = env::var("PROXY_ADDR").unwrap_or("http://127.0.0.1:7777".to_string());
    let client = WebClient::new(api_url.to_string()).map_err(anyhow::Error::from)?;
    Ok(ManagementApi::new(client))
}

pub async fn get_erigon_service() -> anyhow::Result<Service> {
    let api = get_management_api()?;

    api.get_service("erigon").await.map_err(anyhow::Error::from)
}

pub async fn create_erigon_user(username: String, password: String) -> anyhow::Result<()> {
    let api = get_management_api()?;

    let cu = CreateUser { username, password };
    api.create_user("erigon", &cu)
        .await
        .map_err(anyhow::Error::from)?;

    Ok(())
}


pub async fn create_erigon_endpoint(listen_addr: String) -> anyhow::Result<()> {
    let api = get_management_api()?;

    let addresses = Addresses::new(vec![SocketAddr::from_str(&listen_addr).unwrap()]);
    let from_uri = Uri::from_str("/").unwrap();
    let to_uri = Uri::from_str("http://127.0.0.1/").unwrap();
    let cs = CreateService {
        name: "erigon".to_string(),
        server_name: vec![],
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

pub async fn create_erigon_endp(listen_addr: String) -> anyhow::Result<()> {
    let service = match get_erigon_service().await {
        Ok(service) => Some(service),
        Err(_err) => {
            //todo: check if really error or just not exists
            None
        }
    };
    let _service = match service {
        Some(service) => service,
        None => {
            create_erigon_endpoint(listen_addr).await?;
            //Ok(()) => "Created successfully".to_string(),
            //Err(err) => return Err(anyhow!(format!("Error when adding service {err}!")))
            //}
            match get_erigon_service().await {
                Ok(service) => service,
                Err(_err) => return Err(anyhow!("Unknown error when creating service")),
            }
        }
    };
    Ok(())
}
