use tower_governor::{GovernorError, key_extractor::KeyExtractor};

// In production, the reverse proxy sets X-Forwarded-For — that's the primary extraction path.
// ConnectInfo is a fallback for direct connections (not used with the current axum::serve setup).
// LOCALHOST fallback is intentional for tests: each test creates a fresh GovernorLayer with
// an independent bucket, so no rate-limit state leaks between tests.
#[derive(Clone)]
pub struct ClientIpExtractor;

impl KeyExtractor for ClientIpExtractor {
    type Key = std::net::IpAddr;

    fn extract<T>(
        &self,
        req: &axum::http::Request<T>,
    ) -> std::result::Result<Self::Key, GovernorError> {
        let forwarded = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.trim().parse::<std::net::IpAddr>().ok());

        if let Some(ip) = forwarded {
            return Ok(ip);
        }

        if let Some(info) = req
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        {
            return Ok(info.0.ip());
        }

        Ok(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
    }
}
