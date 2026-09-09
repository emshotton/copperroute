pub mod outline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcbError {
    pub section: String,
    pub message: String,
}

impl PcbError {
    pub fn new(section: &str, message: &str) -> PcbError {
        PcbError {
            section: section.to_string(),
            message: message.to_string(),
        }
    }
}

impl std::fmt::Display for PcbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PcbError {}
