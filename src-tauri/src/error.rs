use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// 统一的后端错误类型。
///
/// 该类型刻意保持「平台中立」：具体平台（Windows / macOS / Linux）的实现层
/// 负责把底层错误翻译成这里的语义错误，前端只需按 `code` 做展示与引导。
#[derive(Debug, thiserror::Error)]
pub enum ProcError {
    #[error("进程 {0} 不存在或已退出")]
    ProcessNotFound(u32),
    #[error("权限不足，无法完成操作：{0}（请尝试以管理员身份运行）")]
    AccessDenied(String),
    #[error("枚举系统套接字失败：{0}")]
    SocketTable(String),
    #[error("获取系统信息失败：{0}")]
    System(String),
    #[error("当前平台不支持该操作：{0}")]
    Unsupported(String),
    #[error("{0}")]
    Other(String),
}

impl ProcError {
    /// 稳定的错误码，供前端做分支判断，避免解析中文提示。
    pub fn code(&self) -> &'static str {
        match self {
            ProcError::ProcessNotFound(_) => "process_not_found",
            ProcError::AccessDenied(_) => "access_denied",
            ProcError::SocketTable(_) => "socket_table_failed",
            ProcError::System(_) => "system_error",
            ProcError::Unsupported(_) => "unsupported",
            ProcError::Other(_) => "other",
        }
    }
}

impl Serialize for ProcError {
    /// 序列化成 `{ code, message }`，前端既能直接展示 `message`，
    /// 也能按 `code` 做分支（例如提示「以管理员身份重启」）。
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ProcError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

impl From<std::io::Error> for ProcError {
    fn from(value: std::io::Error) -> Self {
        if value.kind() == std::io::ErrorKind::PermissionDenied {
            ProcError::AccessDenied(value.to_string())
        } else {
            ProcError::System(value.to_string())
        }
    }
}

pub type ProcResult<T> = Result<T, ProcError>;
