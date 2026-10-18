#[cfg(feature = "server")]
static LOGIN_RATE_LIMITER: tokio::sync::OnceCell<RateLimiter> = tokio::sync::OnceCell::const_new();

#[cfg(feature = "server")]
pub struct RateLimiter {
    attempts: std::sync::Arc<
        tokio::sync::Mutex<std::collections::HashMap<String, Vec<std::time::Instant>>>,
    >,
    max_attempts: usize,
    lockout: std::time::Duration,
}

#[cfg(feature = "server")]
impl RateLimiter {
    pub fn new(max_attempts: usize, lockout: std::time::Duration) -> Self {
        Self {
            attempts: std::sync::Arc::new(
                tokio::sync::Mutex::new(std::collections::HashMap::new()),
            ),
            max_attempts,
            lockout,
        }
    }

    pub async fn check_limit(&self, key: &str) -> Result<(), std::time::Duration> {
        let mut map = self.attempts.lock().await;
        let now = std::time::Instant::now();
        let entries = map.entry(key.to_string()).or_default();

        entries.retain(|&t| now.duration_since(t) < self.lockout);

        if entries.len() >= self.max_attempts {
            if let Some(&first) = entries.first() {
                let elapsed = now.duration_since(first);
                if elapsed < self.lockout {
                    return Err(self.lockout - elapsed);
                }
            }
        }
        Ok(())
    }

    pub async fn record_failure(&self, key: &str) {
        let mut map = self.attempts.lock().await;
        let entries = map.entry(key.to_string()).or_default();
        entries.push(std::time::Instant::now());
    }

    pub async fn record_success(&self, key: &str) {
        let mut map = self.attempts.lock().await;
        map.remove(key);
    }
}

#[cfg(feature = "server")]
pub async fn get_rate_limiter() -> &'static RateLimiter {
    LOGIN_RATE_LIMITER
        .get_or_init(|| async { RateLimiter::new(3, std::time::Duration::from_secs(60)) })
        .await
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    /// Verifies IP/Email login brute-force rate limiter:
    /// - Tracks failure count up to the configured threshold (3 attempts).
    /// - Flags the account as rate-limited once threshold is reached.
    /// - Clears and resets failure state upon a successful login.
    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(3, std::time::Duration::from_secs(5));
        let email = "test@example.com";

        // Record 2 failures
        limiter.record_failure(email).await;
        limiter.record_failure(email).await;
        assert!(limiter.check_limit(email).await.is_ok());

        // 3rd failure hits max attempts
        limiter.record_failure(email).await;
        assert!(
            limiter.check_limit(email).await.is_err(),
            "Should be rate limited after 3 attempts"
        );

        // Success clears the rate limit
        limiter.record_success(email).await;
        assert!(
            limiter.check_limit(email).await.is_ok(),
            "Should be allowed after success/clear"
        );
    }
}
