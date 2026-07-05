use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Settings {
    pub addr: String,
    pub valhalla_url: String,
    pub engine_timeout: Duration,
    pub solver_time_limit: Duration,
}

impl Settings {
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Injectable lookup so tests never touch process-global env vars.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let ms = |key: &str, default: u64| {
            Duration::from_millis(lookup(key).and_then(|v| v.parse().ok()).unwrap_or(default))
        };
        Self {
            addr: lookup("STREAMLET_ADDR").unwrap_or_else(|| "0.0.0.0:3000".into()),
            valhalla_url: lookup("STREAMLET_VALHALLA_URL")
                .unwrap_or_else(|| "http://localhost:8002".into()),
            engine_timeout: ms("STREAMLET_ENGINE_TIMEOUT_MS", 10_000),
            solver_time_limit: ms("STREAMLET_SOLVER_TIME_LIMIT_MS", 2_000),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let s = Settings::from_lookup(|_| None);
        assert_eq!(s.addr, "0.0.0.0:3000");
        assert_eq!(s.valhalla_url, "http://localhost:8002");
        assert_eq!(s.engine_timeout.as_millis(), 10_000);
        assert_eq!(s.solver_time_limit.as_millis(), 2_000);
    }

    #[test]
    fn env_overrides_defaults() {
        let s = Settings::from_lookup(|key| match key {
            "STREAMLET_VALHALLA_URL" => Some("http://valhalla:8002".into()),
            "STREAMLET_SOLVER_TIME_LIMIT_MS" => Some("500".into()),
            _ => None,
        });
        assert_eq!(s.valhalla_url, "http://valhalla:8002");
        assert_eq!(s.solver_time_limit.as_millis(), 500);
    }
}
