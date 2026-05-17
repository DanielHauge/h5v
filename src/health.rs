use std::sync::{
    atomic::{AtomicU64, Ordering},
    LazyLock, RwLock,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Fail,
}

impl HealthStatus {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "healthy" | "ok" | "pass" => Some(Self::Healthy),
            "warning" | "warn" => Some(Self::Warning),
            "fail" | "failed" | "error" => Some(Self::Fail),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Warning => "warning",
            Self::Fail => "fail",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthcheckResult {
    pub status: HealthStatus,
    pub message: String,
    pub ui_document: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportedHealthIssue {
    pub source: String,
    pub result: HealthcheckResult,
}

static REPORTED_HEALTH_ISSUES: LazyLock<RwLock<Vec<ReportedHealthIssue>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));
static REPORTED_HEALTH_ISSUES_GENERATION: AtomicU64 = AtomicU64::new(1);

impl HealthcheckResult {
    pub fn healthy(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Healthy,
            message: message.into(),
            ui_document: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Warning,
            message: message.into(),
            ui_document: None,
        }
    }

    pub fn fail(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Fail,
            message: message.into(),
            ui_document: None,
        }
    }
}

impl ReportedHealthIssue {
    pub fn new(source: impl Into<String>, result: HealthcheckResult) -> Self {
        Self {
            source: source.into(),
            result,
        }
    }
}

pub fn reported_health_issues() -> Vec<ReportedHealthIssue> {
    match REPORTED_HEALTH_ISSUES.read() {
        Ok(guard) => guard.clone(),
        Err(error) => error.into_inner().clone(),
    }
}

pub fn push_reported_health_issue(issue: ReportedHealthIssue) {
    match REPORTED_HEALTH_ISSUES.write() {
        Ok(mut guard) => guard.push(issue),
        Err(error) => error.into_inner().push(issue),
    }
    REPORTED_HEALTH_ISSUES_GENERATION.fetch_add(1, Ordering::Relaxed);
}

pub fn clear_reported_health_issues() {
    match REPORTED_HEALTH_ISSUES.write() {
        Ok(mut guard) => guard.clear(),
        Err(error) => error.into_inner().clear(),
    }
    REPORTED_HEALTH_ISSUES_GENERATION.fetch_add(1, Ordering::Relaxed);
}

pub fn reported_health_generation() -> u64 {
    REPORTED_HEALTH_ISSUES_GENERATION.load(Ordering::Relaxed)
}
