#![feature(type_alias_impl_trait, let_else, try_blocks)]
mod api;
mod catchall;
mod common;
mod logging;
mod proxy;
mod session;
mod session_handle;
use crate::common::{endpoint_admin_auth, endpoint_auth, page_auth, SESSION_MAX_AGE};
use crate::session::{SessionMiddleware, SharedSessionStorage};
use anyhow::{Context, Result};
use async_trait::async_trait;
use common::page_admin_auth;
use logging::{log_request_result, span_for_request};
use poem::endpoint::{EmbeddedFileEndpoint, EmbeddedFilesEndpoint};
use poem::listener::{Listener, RustlsCertificate, RustlsConfig, TcpListener};
use poem::middleware::SetHeader;
use poem::session::{CookieConfig, MemoryStorage, ServerSession};
use poem::web::Data;
use poem::{Endpoint, EndpointExt, FromRequest, IntoEndpoint, Route, Server};
use poem_openapi::OpenApiService;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::*;
use warpgate_admin::admin_api_app;
use warpgate_common::{ProtocolName, ProtocolServer, Services, Target, TargetTestError};
use warpgate_web::Assets;

pub const PROTOCOL_NAME: ProtocolName = "HTTP";

pub struct HTTPProtocolServer {
    services: Services,
}

impl HTTPProtocolServer {
    pub async fn new(services: &Services) -> Result<Self> {
        Ok(HTTPProtocolServer {
            services: services.clone(),
        })
    }
}

#[async_trait]
impl ProtocolServer for HTTPProtocolServer {
    async fn run(self, address: SocketAddr) -> Result<()> {
        let admin_api_app = admin_api_app(&self.services).into_endpoint();
        let api_service = OpenApiService::new(
            crate::api::get(),
            "Warpgate HTTP proxy",
            env!("CARGO_PKG_VERSION"),
        )
        .server("/@warpgate/api");
        let ui = api_service.swagger_ui();
        let spec = api_service.spec_endpoint();

        let session_storage =
            SharedSessionStorage(Arc::new(Mutex::new(Box::new(MemoryStorage::default()))));
        let session_middleware = SessionMiddleware::new();

        let app = Route::new()
            .nest(
                "/@warpgate",
                Route::new()
                    .nest("/api/swagger", ui)
                    .nest("/api", api_service)
                    .nest("/api/openapi.json", spec)
                    .nest_no_strip("/assets", EmbeddedFilesEndpoint::<Assets>::new())
                    .nest(
                        "/admin/api",
                        endpoint_auth(endpoint_admin_auth(admin_api_app)),
                    )
                    .at(
                        "/admin",
                        page_auth(page_admin_auth(EmbeddedFileEndpoint::<Assets>::new(
                            "src/admin/index.html",
                        ))),
                    )
                    .at(
                        "",
                        EmbeddedFileEndpoint::<Assets>::new("src/gateway/index.html"),
                    )
                    .around(move |ep, req| async move {
                        let method = req.method().clone();
                        let url = req.original_uri().clone();
                        let response = ep.call(req).await?;
                        log_request_result(&method, &url, &response.status());
                        Ok(response)
                    }),
            )
            .nest_no_strip("/", page_auth(catchall::catchall_endpoint))
            .around(move |ep, req| async move {
                let sm = Data::<&Arc<Mutex<SessionMiddleware>>>::from_request_without_body(&req)
                    .await?
                    .clone();

                let req = { sm.lock().await.process_request(req).await? };

                let span = span_for_request(&req).await?;

                ep.call(req).instrument(span).await
            })
            .with(
                SetHeader::new()
                    .overriding(http::header::STRICT_TRANSPORT_SECURITY, "max-age=31536000"),
            )
            .with(ServerSession::new(
                CookieConfig::default()
                    .secure(false)
                    .max_age(SESSION_MAX_AGE)
                    .name("warpgate-http-session"),
                session_storage.clone(),
            ))
            .data(self.services.clone())
            .data(session_middleware.clone())
            .data(session_storage);

        tokio::spawn(async move {
            loop {
                session_middleware.lock().await.vacuum().await;
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });

        let (certificate, key) = {
            let config = self.services.config.lock().await;
            let certificate_path = config
                .paths_relative_to
                .join(&config.store.http.certificate);
            let key_path = config.paths_relative_to.join(&config.store.http.key);

            (
                std::fs::read(&certificate_path).with_context(|| {
                    format!(
                        "reading SSL certificate from '{}'",
                        certificate_path.display()
                    )
                })?,
                std::fs::read(&key_path).with_context(|| {
                    format!("reading SSL private key from '{}'", key_path.display())
                })?,
            )
        };

        info!(?address, "Listening");
        Server::new(TcpListener::bind(address).rustls(
            RustlsConfig::new().fallback(RustlsCertificate::new().cert(certificate).key(key)),
        ))
        .run(app)
        .await?;

        Ok(())
    }

    async fn test_target(self, _target: Target) -> Result<(), TargetTestError> {
        Ok(())
    }
}

impl Debug for HTTPProtocolServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SSHProtocolServer")
    }
}
