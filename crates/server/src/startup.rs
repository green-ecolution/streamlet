use std::sync::Arc;

use tokio::net::TcpListener;

use crate::config::Settings;
use crate::http::{AppState, router};
use crate::infra::valhalla::ValhallaClient;
use crate::service::SolveService;

pub struct Application {
    pub addr: std::net::SocketAddr,
    listener: TcpListener,
    app: axum::Router,
}

impl Application {
    pub async fn build(settings: &Settings) -> anyhow::Result<Self> {
        let valhalla = ValhallaClient::new(settings.valhalla_url.clone(), settings.engine_timeout)
            .map_err(|e| anyhow::anyhow!("failed to build valhalla client: {e}"))?;
        let solve_service = Arc::new(SolveService::new(
            Arc::new(valhalla),
            settings.solver_time_limit,
        ));
        let state = Arc::new(AppState { solve_service });
        let listener = TcpListener::bind(&settings.addr).await?;
        let addr = listener.local_addr()?;
        Ok(Self {
            addr,
            listener,
            app: router(state),
        })
    }

    pub async fn run(self) -> anyhow::Result<()> {
        tracing::info!(addr = %self.addr, "streamlet listening");
        axum::serve(self.listener, self.app).await?;
        Ok(())
    }
}
