use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::http::{HttpRequest, HttpResponse, Transport, TransportError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 4,
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(8),
        }
    }
}

impl RetryPolicy {
    #[must_use]
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let shift = attempt.saturating_sub(1).min(20);
        let factor = 1u64 << shift;
        let millis = self.base_delay.as_millis() as u64;
        Duration::from_millis(
            (millis.saturating_mul(factor)).min(self.max_delay.as_millis() as u64),
        )
    }
}

#[must_use]
pub fn retry_wait_from(response: &HttpResponse) -> Option<Duration> {
    let value = response.header("retry-after")?;
    let seconds: u64 = value.trim().parse().ok()?;
    Some(Duration::from_secs(seconds.min(60)))
}

#[derive(Debug)]
pub struct RateLimiter {
    min_interval: Duration,
    next_allowed: Mutex<Instant>,
}

impl RateLimiter {
    #[must_use]
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            next_allowed: Mutex::new(Instant::now()),
        }
    }

    pub fn acquire(&self) {
        let mut next = self
            .next_allowed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let now = Instant::now();
        if *next > now {
            let wait = *next - now;
            drop(next);
            std::thread::sleep(wait);
            let mut next = self
                .next_allowed
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *next = Instant::now() + self.min_interval;
            return;
        }

        *next = now + self.min_interval;
    }
}

pub struct RetryTransport<T> {
    inner: T,
    policy: RetryPolicy,
    limiter: Option<Arc<RateLimiter>>,
}

impl<T> RetryTransport<T> {
    #[must_use]
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            policy: RetryPolicy::default(),
            limiter: None,
        }
    }

    #[must_use]
    pub fn with_policy(mut self, policy: RetryPolicy) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn with_limiter(mut self, limiter: Arc<RateLimiter>) -> Self {
        self.limiter = Some(limiter);
        self
    }
}

impl<T: Transport> Transport for RetryTransport<T> {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut attempt = 1;

        loop {
            if let Some(limiter) = &self.limiter {
                limiter.acquire();
            }

            match self.inner.execute(request) {
                Ok(response) => {
                    let retryable = matches!(response.status, 429 | 503);
                    if retryable && attempt < self.policy.max_attempts {
                        let wait = retry_wait_from(&response)
                            .unwrap_or_else(|| self.policy.delay_for(attempt));
                        std::thread::sleep(wait);
                        attempt += 1;
                        continue;
                    }
                    return Ok(response);
                }
                Err(error) => {
                    if attempt < self.policy.max_attempts {
                        std::thread::sleep(self.policy.delay_for(attempt));
                        attempt += 1;
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }
}

impl<T: Transport> Transport for &RetryTransport<T> {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        RetryTransport::execute(self, request)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::time::Instant;

    use super::*;

    #[derive(Default)]
    struct Scripted {
        responses: RefCell<VecDeque<Result<HttpResponse, TransportError>>>,
        requests: RefCell<Vec<HttpRequest>>,
    }

    impl Scripted {
        fn new(responses: Vec<Result<HttpResponse, TransportError>>) -> Self {
            Self {
                responses: RefCell::new(responses.into()),
                requests: RefCell::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<HttpRequest> {
            self.requests.borrow().clone()
        }
    }

    impl Transport for &Scripted {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
            self.requests.borrow_mut().push(request.clone());
            self.responses
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Err(TransportError::new("hết kịch bản")))
        }
    }

    fn fast_policy() -> RetryPolicy {
        RetryPolicy {
            max_attempts: 4,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
        }
    }

    #[test]
    fn a_transport_failure_is_retried_with_backoff() {
        let script = Scripted::new(vec![
            Err(TransportError::new("mất mạng tạm thời")),
            Ok(HttpResponse::new(200).with_body(b"ok".to_vec())),
        ]);

        let transport = RetryTransport::new(&script).with_policy(fast_policy());
        let request = HttpRequest::new(super::super::http::HttpMethod::Get, "https://x.test/");
        let response = transport
            .execute(&request)
            .expect("phải thành công sau retry");

        assert_eq!(response.status, 200);
        assert_eq!(script.requests().len(), 2, "phải gọi lại đúng một lần");
    }

    #[test]
    fn a_rate_limit_honours_the_retry_after_header() {
        let script = Scripted::new(vec![
            Ok(HttpResponse::new(429).with_header("Retry-After", "0")),
            Ok(HttpResponse::new(200)),
        ]);

        let transport = RetryTransport::new(&script).with_policy(fast_policy());
        let request = HttpRequest::new(super::super::http::HttpMethod::Get, "https://x.test/");

        let response = transport.execute(&request).expect("thành công");
        assert_eq!(response.status, 200);
        assert_eq!(script.requests().len(), 2);
    }

    #[test]
    fn exhausting_the_attempts_returns_the_last_response() {
        let script = Scripted::new(vec![
            Ok(HttpResponse::new(503)),
            Ok(HttpResponse::new(503)),
            Ok(HttpResponse::new(503)),
            Ok(HttpResponse::new(503)),
        ]);

        let transport = RetryTransport::new(&script).with_policy(fast_policy());
        let request = HttpRequest::new(super::super::http::HttpMethod::Get, "https://x.test/");

        let response = transport.execute(&request).expect("vẫn trả response");
        assert_eq!(response.status, 503);
        assert_eq!(script.requests().len(), 4, "đủ 4 lần thử rồi dừng");
    }

    #[test]
    fn a_permanent_transport_failure_gives_up_after_the_policy_runs_out() {
        let script = Scripted::new(vec![
            Err(TransportError::new("mất mạng hoàn toàn"));
            4
        ]);

        let transport = RetryTransport::new(&script).with_policy(fast_policy());
        let request = HttpRequest::new(super::super::http::HttpMethod::Get, "https://x.test/");

        let error = transport.execute(&request).expect_err("phải bỏ cuộc");
        assert!(error.message.contains("mất mạng"), "gặp: {error:?}");
        assert_eq!(script.requests().len(), 4);
    }

    #[test]
    fn a_successful_response_is_returned_without_any_retry() {
        let script = Scripted::new(vec![Ok(HttpResponse::new(200))]);

        let transport = RetryTransport::new(&script).with_policy(fast_policy());
        let request = HttpRequest::new(super::super::http::HttpMethod::Get, "https://x.test/");

        let response = transport.execute(&request).expect("thành công");
        assert_eq!(response.status, 200);
        assert_eq!(script.requests().len(), 1);
    }

    #[test]
    fn the_backoff_grows_and_stays_capped() {
        let policy = RetryPolicy {
            max_attempts: 8,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(400),
        };

        assert_eq!(policy.delay_for(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for(3), Duration::from_millis(400));
        assert_eq!(policy.delay_for(4), Duration::from_millis(400));
        assert_eq!(policy.delay_for(50), Duration::from_millis(400));
    }

    #[test]
    fn the_limiter_spaces_requests_apart() {
        let limiter = RateLimiter::new(Duration::from_millis(60));

        let start = Instant::now();
        limiter.acquire();
        limiter.acquire();
        limiter.acquire();

        assert!(
            start.elapsed() >= Duration::from_millis(100),
            "ba lần acquire phải cách nhau ít nhất hai khoảng: {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn a_missing_retry_after_header_falls_back_to_the_policy_delay() {
        let response = HttpResponse::new(429);
        assert_eq!(retry_wait_from(&response), None);

        let response = HttpResponse::new(429).with_header("Retry-After", "khong-phai-so");
        assert_eq!(retry_wait_from(&response), None);
    }
}
