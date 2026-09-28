//! Где что лежит, и чем служба для разработки отличается от рабочей.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    /// Настройки, ключи, журнал: `%ProgramData%\klick` или `.dev-data` в папке проекта.
    pub data: PathBuf,
    /// Домашняя папка ядра: конфиг, серверы, наборы, база стран.
    pub core_home: PathBuf,
    pub providers: PathBuf,
    pub sets: PathBuf,
    pub logs: PathBuf,
    pub settings: PathBuf,
    pub secrets: PathBuf,
    /// Файлы, которые приходят с установкой: ядро, наборы, каталог сервисов.
    pub resources: PathBuf,
    pub core_exe: PathBuf,
}

impl Paths {
    pub fn new(data: PathBuf, resources: PathBuf) -> Self {
        let core_home = data.join("core");
        Paths {
            providers: core_home.join("providers"),
            sets: core_home.join("sets"),
            core_exe: resources.join("core").join("mihomo.exe"),
            logs: data.join("logs"),
            settings: data.join("settings.json"),
            secrets: data.join("secrets.bin"),
            core_home,
            data,
            resources,
        }
    }

    /// Рабочая служба: данные в `%ProgramData%\klick`, ресурсы рядом с exe.
    pub fn production() -> Result<Self> {
        let program_data = std::env::var_os("ProgramData").context("нет переменной ProgramData")?;
        let exe = std::env::current_exe()?;
        let dir = exe.parent().context("нет папки exe")?.to_path_buf();
        Ok(Self::new(PathBuf::from(program_data).join("klick"), dir.join("resources")))
    }

    /// Разработка: ищем папку проекта с `resources` вверх от exe.
    pub fn development(root: Option<PathBuf>) -> Result<Self> {
        let root = match root {
            Some(r) => r,
            None => find_root(&std::env::current_exe()?)?,
        };
        Ok(Self::new(root.join(".dev-data"), root.join("resources")))
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for d in [&self.data, &self.core_home, &self.providers, &self.sets, &self.logs] {
            std::fs::create_dir_all(d).with_context(|| format!("не создать {}", d.display()))?;
        }
        Ok(())
    }

    /// Путь файла серверов подключения для конфига ядра (относительно домашней папки).
    pub fn provider_rel(id: &str) -> String {
        format!("providers/{id}.txt")
    }

    pub fn provider_file(&self, id: &str) -> PathBuf {
        self.providers.join(format!("{id}.txt"))
    }
}

fn find_root(exe: &Path) -> Result<PathBuf> {
    let mut dir = exe.parent();
    while let Some(d) = dir {
        if d.join("resources").join("core").join("mihomo.exe").exists() {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }
    bail!("не нашёл папку проекта с resources\\core\\mihomo.exe выше {}", exe.display())
}

/// Чем отличаются рабочая служба и служба для разработки.
#[derive(Clone, Debug)]
pub struct Profile {
    pub dev: bool,
    pub pipe: String,
    pub core_pipe: String,
    pub tester_pipe: String,
    pub mixed_port: u16,
    /// Можно ли поднимать адаптер TUN. В разработке — только с флагом.
    pub tun_allowed: bool,
    /// Можно ли трогать брандмауэр. В разработке — только с флагом и правами администратора.
    pub wfp_allowed: bool,
    pub core_log_level: String,
}

impl Profile {
    pub fn production() -> Self {
        Profile {
            dev: false,
            pipe: klick_proto::PIPE.into(),
            core_pipe: r"\\.\pipe\klick-core".into(),
            tester_pipe: r"\\.\pipe\klick-tester".into(),
            mixed_port: 7890,
            tun_allowed: true,
            wfp_allowed: true,
            core_log_level: "warning".into(),
        }
    }

    pub fn development(allow_tun: bool, allow_wfp: bool) -> Self {
        Profile {
            dev: true,
            pipe: klick_proto::PIPE_DEV.into(),
            core_pipe: r"\\.\pipe\klick-dev-core".into(),
            tester_pipe: r"\\.\pipe\klick-dev-tester".into(),
            mixed_port: 17890,
            tun_allowed: allow_tun,
            wfp_allowed: allow_wfp,
            core_log_level: "info".into(),
        }
    }
}
