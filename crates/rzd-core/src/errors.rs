use crate::engine::EngineError;
use std::fmt;

#[derive(Debug)]
pub enum RzdError {
    // Core engine errors
    Engine(EngineError),

    // Validation errors
    Network(String),
    Scenario(String),
    Export(String),

    // Configuration and setup errors
    Config(String),
    Injection(String),

    // Performance and resource errors
    Timeout(String),
    ResourceExhaustion(String),

    // File system and IO errors
    FileIo(std::io::Error),
    Json(serde_json::Error),

    // External API errors
    ExternalApi(String),
    RateLimitExceeded(String),

    // Context tracking
    WithContext {
        error: Box<RzdError>,
        context: Vec<String>,
    },
}

impl fmt::Display for RzdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RzdError::Engine(e) => write!(f, "{}", e),
            RzdError::Network(msg) => write!(f, "Network error: {}", msg),
            RzdError::Scenario(msg) => write!(f, "Scenario error: {}", msg),
            RzdError::Export(msg) => write!(f, "Export error: {}", msg),
            RzdError::Config(msg) => write!(f, "Config error: {}", msg),
            RzdError::Injection(msg) => write!(f, "Injection error: {}", msg),
            RzdError::Timeout(msg) => write!(f, "Timeout error: {}", msg),
            RzdError::ResourceExhaustion(msg) => write!(f, "Resource exhausted: {}", msg),
            RzdError::FileIo(e) => write!(f, "File I/O error: {}", e),
            RzdError::Json(e) => write!(f, "JSON error: {}", e),
            RzdError::ExternalApi(msg) => write!(f, "External API error: {}", msg),
            RzdError::RateLimitExceeded(msg) => write!(f, "Rate limit exceeded: {}", msg),
            RzdError::WithContext { error, context } => {
                write!(f, "{}", error)?;
                if !context.is_empty() {
                    write!(f, " (context: {:?})", context)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for RzdError {}

// Convenience constructors for common error types
impl RzdError {
    pub fn with_context(self, additional: String) -> Self {
        match self {
            RzdError::WithContext { error, context } => {
                let mut ctx = context;
                ctx.push(additional);
                RzdError::WithContext {
                    error,
                    context: ctx,
                }
            }
            _ => RzdError::WithContext {
                error: Box::new(self),
                context: vec![additional],
            },
        }
    }

    pub fn network(msg: &str) -> Self {
        RzdError::Network(msg.to_string())
    }

    pub fn scenario(msg: &str) -> Self {
        RzdError::Scenario(msg.to_string())
    }

    pub fn export(msg: &str) -> Self {
        RzdError::Export(msg.to_string())
    }

    pub fn config(msg: &str) -> Self {
        RzdError::Config(msg.to_string())
    }

    pub fn timeout(msg: &str) -> Self {
        RzdError::Timeout(msg.to_string())
    }

    pub fn resource_exhausted(msg: &str) -> Self {
        RzdError::ResourceExhaustion(msg.to_string())
    }

    pub fn external_api(msg: &str) -> Self {
        RzdError::ExternalApi(msg.to_string())
    }

    pub fn rate_limited(msg: &str) -> Self {
        RzdError::RateLimitExceeded(msg.to_string())
    }
}

// Convert from other error types
impl From<EngineError> for RzdError {
    fn from(e: EngineError) -> Self {
        RzdError::Engine(e)
    }
}

impl From<std::io::Error> for RzdError {
    fn from(e: std::io::Error) -> Self {
        RzdError::FileIo(e)
    }
}

impl From<serde_json::Error> for RzdError {
    fn from(e: serde_json::Error) -> Self {
        RzdError::Json(e)
    }
}
