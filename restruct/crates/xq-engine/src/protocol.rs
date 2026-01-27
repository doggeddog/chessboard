//! 引擎协议类型与基础工具。

use std::fmt;

/// 支持的引擎协议。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineProtocol {
    Uci,
    Ucci,
}

impl EngineProtocol {
    /// 协议名称（用于日志与展示）。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uci => "uci",
            Self::Ucci => "ucci",
        }
    }
}

impl fmt::Display for EngineProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::EngineProtocol;

    #[test]
    fn protocol_as_str_is_stable() {
        assert_eq!(EngineProtocol::Uci.as_str(), "uci");
        assert_eq!(EngineProtocol::Ucci.as_str(), "ucci");
    }
}
