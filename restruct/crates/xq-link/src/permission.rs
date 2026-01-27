#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputPermissionStatus {
    Granted,
    Denied,
    NotSupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionCheck {
    pub status: InputPermissionStatus,
    pub guidance: Option<&'static str>,
}

pub fn check_input_permission() -> PermissionCheck {
    #[cfg(target_os = "macos")]
    {
        if is_macos_trusted() {
            PermissionCheck {
                status: InputPermissionStatus::Granted,
                guidance: None,
            }
        } else {
            PermissionCheck {
                status: InputPermissionStatus::Denied,
                guidance: Some(
                    "请在 系统设置 > 隐私与安全性 > 辅助功能 与 输入监控 中授权本应用。",
                ),
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        PermissionCheck {
            status: InputPermissionStatus::NotSupported,
            guidance: None,
        }
    }
}

#[cfg(target_os = "macos")]
fn is_macos_trusted() -> bool {
    use std::ffi::c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    }

    unsafe { AXIsProcessTrustedWithOptions(std::ptr::null()) != 0 }
}
