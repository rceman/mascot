//! Validation report types (`mascot-character-validation/1`).

use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Fail,
    Warn,
    Info,
}

impl Severity {
    pub fn tag(self) -> &'static str {
        match self {
            Severity::Fail => "FAIL",
            Severity::Warn => "WARN",
            Severity::Info => "INFO",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub code: String,
    pub subject: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
}

impl Finding {
    pub fn new(
        sev: Severity,
        code: impl Into<String>,
        subject: impl Into<String>,
        msg: impl Into<String>,
    ) -> Self {
        Finding {
            severity: sev,
            code: code.into(),
            subject: subject.into(),
            message: msg.into(),
            value: None,
            limit: None,
        }
    }

    pub fn v(mut self, v: f64, limit: f64) -> Self {
        self.value = Some((v * 1e4).round() / 1e4);
        self.limit = Some(limit);
        self
    }
}

/// Gate-18 artifact identity: which tool produced this file.
#[derive(Serialize)]
pub struct Tool {
    pub name: &'static str,
    pub version: &'static str,
}

pub fn tool() -> Tool {
    Tool {
        name: "mascotctl",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[derive(Serialize)]
pub struct ProfileRef {
    pub id: String,
    pub revision: u32,
    pub sha256: String,
}

#[derive(Serialize)]
pub struct ImageRef {
    pub sha256: String,
    pub size: [u32; 2],
}

#[derive(Serialize)]
pub struct Counts {
    pub fail: usize,
    pub warn: usize,
    pub info: usize,
}

#[derive(Serialize)]
pub struct ValidationReport {
    pub format: &'static str,
    pub source: String,
    pub character_id: String,
    pub profile: ProfileRef,
    pub image: ImageRef,
    pub result: String,
    pub counts: Counts,
    pub findings: Vec<Finding>,
    pub tool: Tool,
}
