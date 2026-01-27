//! 引擎参数系统（Profile）与搜索参数。

use std::path::{Path, PathBuf};

use crate::protocol::EngineProtocol;

/// 搜索参数：用于 `go` 指令。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchParams {
    pub depth: Option<u32>,
    pub movetime_ms: Option<u64>,
    pub nodes: Option<u64>,
    pub infinite: bool,
}

impl SearchParams {
    /// 构造一个仅限制深度的搜索参数。
    #[must_use]
    pub const fn with_depth(depth: u32) -> Self {
        Self {
            depth: Some(depth),
            movetime_ms: None,
            nodes: None,
            infinite: false,
        }
    }

    /// 构造一个仅限制时间（毫秒）的搜索参数。
    #[must_use]
    pub const fn with_movetime(movetime_ms: u64) -> Self {
        Self {
            depth: None,
            movetime_ms: Some(movetime_ms),
            nodes: None,
            infinite: false,
        }
    }

    /// 将搜索参数映射为协议 `go` 指令。
    #[must_use]
    pub fn to_go_command(&self, protocol: EngineProtocol) -> String {
        let mut parts = vec!["go".to_string()];

        if let Some(depth) = self.depth {
            parts.push("depth".to_string());
            parts.push(depth.to_string());
        }

        if let Some(movetime) = self.movetime_ms {
            // UCI 通常使用 movetime，UCCI 常见实现更偏向 time。
            let key = match protocol {
                EngineProtocol::Uci => "movetime",
                EngineProtocol::Ucci => "time",
            };
            parts.push(key.to_string());
            parts.push(movetime.to_string());
        }

        if let Some(nodes) = self.nodes {
            parts.push("nodes".to_string());
            parts.push(nodes.to_string());
        }

        if self.infinite {
            parts.push("infinite".to_string());
        }

        parts.join(" ")
    }
}

/// 通用引擎选项（会映射为 `setoption`）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineOptions {
    pub threads: Option<u32>,
    pub hash_mb: Option<u32>,
    pub eval_file: Option<PathBuf>,
}

/// 原始引擎选项（name/value）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOption {
    pub name: String,
    pub value: String,
}

impl EngineOption {
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// 引擎 Profile：协议、路径、参数快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineProfile {
    pub name: String,
    pub protocol: EngineProtocol,
    pub engine_path: PathBuf,
    pub options: EngineOptions,
    pub search: SearchParams,
    pub extra_options: Vec<EngineOption>,
}

impl EngineProfile {
    /// 创建一个新的 Profile。
    #[must_use]
    pub fn new(name: impl Into<String>, protocol: EngineProtocol, engine_path: impl AsRef<Path>) -> Self {
        Self {
            name: name.into(),
            protocol,
            engine_path: engine_path.as_ref().to_path_buf(),
            options: EngineOptions::default(),
            search: SearchParams::default(),
            extra_options: Vec::new(),
        }
    }

    /// 设置线程数。
    #[must_use]
    pub fn with_threads(mut self, threads: u32) -> Self {
        self.options.threads = Some(threads);
        self
    }

    /// 设置 Hash（MB）。
    #[must_use]
    pub fn with_hash_mb(mut self, hash_mb: u32) -> Self {
        self.options.hash_mb = Some(hash_mb);
        self
    }

    /// 设置 Eval/NNUE 文件路径。
    #[must_use]
    pub fn with_eval_file(mut self, eval_file: impl AsRef<Path>) -> Self {
        self.options.eval_file = Some(eval_file.as_ref().to_path_buf());
        self
    }

    /// 设置搜索深度。
    #[must_use]
    pub fn with_search_depth(mut self, depth: u32) -> Self {
        self.search.depth = Some(depth);
        self
    }

    /// 设置搜索时间（毫秒）。
    #[must_use]
    pub fn with_search_movetime(mut self, movetime_ms: u64) -> Self {
        self.search.movetime_ms = Some(movetime_ms);
        self
    }

    /// 增加一个原始 setoption。
    #[must_use]
    pub fn with_extra_option(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_options.push(EngineOption::new(name, value));
        self
    }

    /// 生成所有 `setoption` 指令（协议相关映射在这里统一处理）。
    #[must_use]
    pub fn to_setoption_commands(&self) -> Vec<String> {
        let mut commands = Vec::new();

        if let Some(threads) = self.options.threads {
            commands.push(format!("setoption name Threads value {threads}"));
        }

        if let Some(hash_mb) = self.options.hash_mb {
            commands.push(format!("setoption name Hash value {hash_mb}"));
        }

        if let Some(path) = &self.options.eval_file {
            // 常见引擎（含 Pikafish）使用 EvalFile 作为 NNUE 文件选项名。
            let value = path.to_string_lossy();
            commands.push(format!("setoption name EvalFile value {value}"));
        }

        for opt in &self.extra_options {
            commands.push(format!("setoption name {} value {}", opt.name, opt.value));
        }

        commands
    }
}

#[cfg(test)]
mod tests {
    use super::{EngineProfile, SearchParams};
    use crate::protocol::EngineProtocol;

    #[test]
    fn go_command_maps_movetime_by_protocol() {
        let params = SearchParams::with_movetime(1500);
        assert_eq!(params.to_go_command(EngineProtocol::Uci), "go movetime 1500");
        assert_eq!(params.to_go_command(EngineProtocol::Ucci), "go time 1500");
    }

    #[test]
    fn profile_generates_setoption_commands() {
        let profile = EngineProfile::new("test", EngineProtocol::Uci, "/bin/true")
            .with_threads(4)
            .with_hash_mb(256)
            .with_extra_option("MultiPV", "3");

        let cmds = profile.to_setoption_commands();
        assert!(cmds.iter().any(|c| c == "setoption name Threads value 4"));
        assert!(cmds.iter().any(|c| c == "setoption name Hash value 256"));
        assert!(cmds.iter().any(|c| c == "setoption name MultiPV value 3"));
    }
}
