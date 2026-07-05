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
        axum::serve(self.listener, self.app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        Ok(())
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received, draining requests");
}
