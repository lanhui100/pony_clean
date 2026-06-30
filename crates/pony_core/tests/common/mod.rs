use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static TRACING_INIT: OnceLock<()> = OnceLock::new();

pub fn init_logging() {
    TRACING_INIT.get_or_init(|| {
        tracing_subscriber::fmt().with_test_writer().init();
    });
}

/// 测试环境模拟：创建 Windows 目录结构用于集成测试
pub struct TestEnv {
    pub root: PathBuf,
    _tmp: tempfile::TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().to_path_buf();
        Self { root, _tmp: tmp }
    }

    pub fn create_windows_tree(&self) {
        let r = &self.root;
        // 系统路径
        create_dirs(&r.join("Windows\\System32\\LogFiles"));
        create_dirs(&r.join("Windows\\System32\\sru"));
        create_dirs(&r.join("Windows\\System32\\oobe\\info"));
        create_dirs(&r.join("Windows\\System32\\NtmsData"));
        create_dirs(&r.join("Windows\\System32\\Macromed\\Flash"));
        create_dirs(&r.join("Windows\\System32\\spool\\SERVERS"));
        create_dirs(&r.join("Windows\\System32\\MsDtc\\Trace"));
        create_dirs(&r.join("Windows\\Logs"));
        create_dirs(&r.join("Windows\\Temp"));
        create_dirs(&r.join("Windows\\Prefetch"));
        create_dirs(&r.join("Windows\\Downloaded Program Files"));
        create_dirs(&r.join("Windows\\SoftwareDistribution\\Download"));
        create_dirs(&r.join("Windows\\SoftwareDistribution\\DataStore"));
        // 用户路径
        let user = r.join("Users\\TestUser");
        create_dirs(&user.join("AppData\\Local\\Temp"));
        create_dirs(&user.join("AppData\\Local\\Microsoft\\Windows\\WER"));
        create_dirs(&user.join("AppData\\Local\\Microsoft\\Windows\\INetCache\\IE"));
        create_dirs(&user.join("AppData\\Local\\Microsoft\\Windows\\AppCache"));
        create_dirs(&user.join("AppData\\Local\\Microsoft\\Windows\\Caches"));
        create_dirs(&user.join("AppData\\Local\\CrashDumps"));
        create_dirs(&user.join("AppData\\Local\\Microsoft\\Media Player"));
        create_dirs(&user.join("AppData\\Local\\Google\\Chrome\\User Data\\Default\\Cache"));
        create_dirs(&user.join("AppData\\Local\\Microsoft\\Edge\\User Data\\Default\\Cache"));
        create_dirs(&user.join("AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\abcd.default-release\\cache2\\entries"));
        create_dirs(&user.join("Downloads"));
        // UWP 包
        for i in 0..3 {
            create_dirs(&r.join(format!("Users\\TestUser\\AppData\\Local\\Packages\\Pkg{i}\\AC\\Temp")));
        }
        // 系统级
        create_dirs(&r.join("ProgramData\\Microsoft\\Windows\\WER"));
        create_dirs(&r.join("$Recycle.Bin"));
    }

    pub fn apply_env(&self) {
        let r = &self.root;
        // set_var is unsafe in Rust 2024; we accept the risk in test code
        unsafe {
            std::env::set_var("SystemRoot", r.join("Windows").to_str().unwrap());
            std::env::set_var("TEMP", r.join("Users\\TestUser\\AppData\\Local\\Temp").to_str().unwrap());
            std::env::set_var("LOCALAPPDATA", r.join("Users\\TestUser\\AppData\\Local").to_str().unwrap());
            std::env::set_var("APPDATA", r.join("Users\\TestUser\\AppData\\Roaming").to_str().unwrap());
            std::env::set_var("USERPROFILE", r.join("Users\\TestUser").to_str().unwrap());
            std::env::set_var("ALLUSERSPROFILE", r.join("ProgramData").to_str().unwrap());
            std::env::set_var("PUBLIC", r.join("Users\\Public").to_str().unwrap());
        }
    }
}

fn create_dirs(p: &Path) { std::fs::create_dir_all(p).unwrap(); }
