use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguagePreference {
    #[default]
    System,
    English,
    SimplifiedChinese,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    English,
    SimplifiedChinese,
}

impl LanguagePreference {
    pub fn resolve(self) -> Locale {
        match self {
            Self::English => Locale::English,
            Self::SimplifiedChinese => Locale::SimplifiedChinese,
            Self::System => match sys_locale::get_locale()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .as_str()
            {
                locale if locale.starts_with("zh") => Locale::SimplifiedChinese,
                _ => Locale::English,
            },
        }
    }
}

#[derive(Clone, Copy)]
pub enum Text {
    NotSignedIn,
    Email,
    EmailPlaceholder,
    Password,
    PasswordPlaceholder,
    SignIn,
    SaveTo,
    FolderPlaceholder,
    Browse,
    Workers,
    AutoDecrypt,
    Language,
    Appearance,
    DarkTheme,
    LightTheme,
    FollowSystem,
    English,
    SimplifiedChinese,
    SystemProxy,
    ManualProxy,
    HttpProxy,
    Socks5Proxy,
    ProxyAddress,
    ProxyAddressPlaceholder,
    SystemProxyFallback,
    Refresh,
    SelectAll,
    ClearSelection,
    DownloadSelected,
    SignInPrompt,
    EmptyCredentials,
    SigningIn,
    SignInFailed,
    BooksLoaded,
    RefreshPrompt,
    Refreshing,
    RefreshFailed,
    SelectBooks,
    QueueRunning,
    QueueFinished,
    Ready,
    Queued,
    Downloading,
    Failed,
    DecryptedFile,
    EncryptedFile,
    ProxySettingSaved,
    LoginTitle,
    LoginSubtitle,
    LibraryNav,
    DownloadsNav,
    SettingsNav,
    Logout,
    Pause,
    Resume,
    PauseRequested,
    ResumeRequested,
    NoDownloads,
    NoBooks,
    SelectedCount,
    Account,
    DownloadFolder,
    Network,
    Preferences,
    DownloadsSubtitle,
    SettingsSubtitle,
    SearchBooks,
    Paused,
    PauseAll,
    ResumeAll,
    ClearCompleted,
    DownloadBehavior,
    SingleThread,
    ExperimentalParallel,
    DownloadModeHint,
    NextQueueHint,
    KeepEncrypted,
    KeepEncryptedHint,
    Exporting,
    CleanupFailed,
    On,
    Off,
    ShowInFolder,
    DeleteFile,
    DeletingFile,
    MovedToTrash,
    PermanentlyDeleted,
    FileMissing,
    RevealFailed,
    DeleteFailed,
    DeletePolicy,
}

pub fn text(locale: Locale, key: Text) -> &'static str {
    match (locale, key) {
        (Locale::English, Text::NotSignedIn) => "Not signed in",
        (Locale::English, Text::Email) => "EMAIL",
        (Locale::English, Text::EmailPlaceholder) => "account email",
        (Locale::English, Text::Password) => "PASSWORD",
        (Locale::English, Text::PasswordPlaceholder) => "password",
        (Locale::English, Text::SignIn) => "Sign in",
        (Locale::English, Text::SaveTo) => "SAVE TO",
        (Locale::English, Text::FolderPlaceholder) => "Choose a folder",
        (Locale::English, Text::Browse) => "Browse…",
        (Locale::English, Text::Workers) => "WORKERS",
        (Locale::English, Text::AutoDecrypt) => "Export decrypted EPUB",
        (Locale::English, Text::Language) => "LANGUAGE",
        (Locale::English, Text::Appearance) => "APPEARANCE",
        (Locale::English, Text::DarkTheme) => "Dark",
        (Locale::English, Text::LightTheme) => "Light",
        (Locale::English, Text::FollowSystem) => "System",
        (Locale::English, Text::English) => "English",
        (Locale::English, Text::SimplifiedChinese) => "简体中文",
        (Locale::English, Text::SystemProxy) => "System",
        (Locale::English, Text::ManualProxy) => "Manual",
        (Locale::English, Text::HttpProxy) => "HTTP",
        (Locale::English, Text::Socks5Proxy) => "SOCKS5",
        (Locale::English, Text::ProxyAddress) => "ADDRESS",
        (Locale::English, Text::ProxyAddressPlaceholder) => "host:port",
        (Locale::English, Text::SystemProxyFallback) => {
            "Uses OS proxy settings, then proxy environment variables"
        }
        (Locale::English, Text::Refresh) => "Refresh",
        (Locale::English, Text::SelectAll) => "Select all",
        (Locale::English, Text::ClearSelection) => "Clear selection",
        (Locale::English, Text::DownloadSelected) => "Download selected",
        (Locale::English, Text::SignInPrompt) => "Sign in to load your purchased books.",
        (Locale::English, Text::EmptyCredentials) => "Enter both email and password.",
        (Locale::English, Text::SigningIn) => "Signing in and loading your bookshelf…",
        (Locale::English, Text::SignInFailed) => "Sign in failed",
        (Locale::English, Text::BooksLoaded) => "purchased books loaded",
        (Locale::English, Text::RefreshPrompt) => "Sign in before refreshing the bookshelf.",
        (Locale::English, Text::Refreshing) => "Refreshing purchased books…",
        (Locale::English, Text::RefreshFailed) => "Could not load books",
        (Locale::English, Text::SelectBooks) => "Select at least one book.",
        (Locale::English, Text::QueueRunning) => "Wait for the current download queue to finish.",
        (Locale::English, Text::QueueFinished) => "Download queue finished.",
        (Locale::English, Text::Ready) => "Ready",
        (Locale::English, Text::Queued) => "Queued",
        (Locale::English, Text::Downloading) => "Downloading",
        (Locale::English, Text::Failed) => "Failed",
        (Locale::English, Text::DecryptedFile) => "Ready",
        (Locale::English, Text::EncryptedFile) => "Downloaded",
        (Locale::English, Text::ProxySettingSaved) => "Proxy settings saved.",
        (Locale::English, Text::LoginTitle) => "Sign in to Endao",
        (Locale::English, Text::LoginSubtitle) => "Access the books in your purchased library.",
        (Locale::English, Text::LibraryNav) => "Library",
        (Locale::English, Text::DownloadsNav) => "Downloads",
        (Locale::English, Text::SettingsNav) => "Settings",
        (Locale::English, Text::Logout) => "Sign out",
        (Locale::English, Text::Pause) => "Pause",
        (Locale::English, Text::Resume) => "Resume",
        (Locale::English, Text::PauseRequested) => "Downloads paused. Your progress is saved.",
        (Locale::English, Text::ResumeRequested) => "Downloads resumed.",
        (Locale::English, Text::NoDownloads) => "No downloads yet",
        (Locale::English, Text::NoBooks) => "No purchased books found",
        (Locale::English, Text::SelectedCount) => "selected",
        (Locale::English, Text::Account) => "ACCOUNT",
        (Locale::English, Text::DownloadFolder) => "DOWNLOAD FOLDER",
        (Locale::English, Text::Network) => "NETWORK",
        (Locale::English, Text::Preferences) => "PREFERENCES",
        (Locale::English, Text::DownloadsSubtitle) => "Track progress and manage your queue.",
        (Locale::English, Text::SettingsSubtitle) => "Download location, language and network.",
        (Locale::English, Text::SearchBooks) => "Search title or author",
        (Locale::English, Text::Paused) => "Paused",
        (Locale::English, Text::PauseAll) => "Pause all",
        (Locale::English, Text::ResumeAll) => "Resume all",
        (Locale::English, Text::ClearCompleted) => "Clear completed",
        (Locale::English, Text::DownloadBehavior) => "DOWNLOAD & EXPORT",
        (Locale::English, Text::SingleThread) => "Single thread",
        (Locale::English, Text::ExperimentalParallel) => "Parallel · experimental",
        (Locale::English, Text::DownloadModeHint) => {
            "One book at a time. Experimental mode uses multiple connections per book."
        }
        (Locale::English, Text::NextQueueHint) => "Changes apply to the next queue.",
        (Locale::English, Text::KeepEncrypted) => "Keep encrypted originals",
        (Locale::English, Text::KeepEncryptedHint) => {
            "Off by default: remove .books.epub after a successful export. Failed exports keep the source."
        }
        (Locale::English, Text::Exporting) => "Exporting EPUB…",
        (Locale::English, Text::CleanupFailed) => "Exported; could not remove encrypted source",
        (Locale::English, Text::On) => "On",
        (Locale::English, Text::Off) => "Off",
        (Locale::English, Text::ShowInFolder) => "Show in folder",
        (Locale::English, Text::DeleteFile) => "Delete file",
        (Locale::English, Text::DeletingFile) => "Deleting file…",
        (Locale::English, Text::MovedToTrash) => "Moved to recycle bin",
        (Locale::English, Text::PermanentlyDeleted) => "Permanently deleted",
        (Locale::English, Text::FileMissing) => "File no longer exists",
        (Locale::English, Text::RevealFailed) => "Could not show file",
        (Locale::English, Text::DeleteFailed) => "Could not delete file",
        (Locale::English, Text::DeletePolicy) => {
            "Delete file moves the output to the recycle bin, or permanently deletes it if recycling fails."
        }
        (Locale::SimplifiedChinese, Text::NotSignedIn) => "未登录",
        (Locale::SimplifiedChinese, Text::Email) => "邮箱",
        (Locale::SimplifiedChinese, Text::EmailPlaceholder) => "账号邮箱",
        (Locale::SimplifiedChinese, Text::Password) => "密码",
        (Locale::SimplifiedChinese, Text::PasswordPlaceholder) => "输入密码",
        (Locale::SimplifiedChinese, Text::SignIn) => "登录",
        (Locale::SimplifiedChinese, Text::SaveTo) => "保存到",
        (Locale::SimplifiedChinese, Text::FolderPlaceholder) => "选择保存目录",
        (Locale::SimplifiedChinese, Text::Browse) => "浏览…",
        (Locale::SimplifiedChinese, Text::Workers) => "并发数",
        (Locale::SimplifiedChinese, Text::AutoDecrypt) => "导出解密 EPUB",
        (Locale::SimplifiedChinese, Text::Language) => "语言",
        (Locale::SimplifiedChinese, Text::Appearance) => "外观",
        (Locale::SimplifiedChinese, Text::DarkTheme) => "深色",
        (Locale::SimplifiedChinese, Text::LightTheme) => "浅色",
        (Locale::SimplifiedChinese, Text::FollowSystem) => "跟随系统",
        (Locale::SimplifiedChinese, Text::English) => "English",
        (Locale::SimplifiedChinese, Text::SimplifiedChinese) => "简体中文",
        (Locale::SimplifiedChinese, Text::SystemProxy) => "系统",
        (Locale::SimplifiedChinese, Text::ManualProxy) => "手动",
        (Locale::SimplifiedChinese, Text::HttpProxy) => "HTTP",
        (Locale::SimplifiedChinese, Text::Socks5Proxy) => "SOCKS5",
        (Locale::SimplifiedChinese, Text::ProxyAddress) => "地址",
        (Locale::SimplifiedChinese, Text::ProxyAddressPlaceholder) => "主机:端口",
        (Locale::SimplifiedChinese, Text::SystemProxyFallback) => {
            "读取系统代理；不可用时使用代理环境变量"
        }
        (Locale::SimplifiedChinese, Text::Refresh) => "刷新",
        (Locale::SimplifiedChinese, Text::SelectAll) => "全选",
        (Locale::SimplifiedChinese, Text::ClearSelection) => "清除选择",
        (Locale::SimplifiedChinese, Text::DownloadSelected) => "下载所选",
        (Locale::SimplifiedChinese, Text::SignInPrompt) => "登录后加载已购书库。",
        (Locale::SimplifiedChinese, Text::EmptyCredentials) => "请输入邮箱和密码。",
        (Locale::SimplifiedChinese, Text::SigningIn) => "正在登录并加载书库…",
        (Locale::SimplifiedChinese, Text::SignInFailed) => "登录失败",
        (Locale::SimplifiedChinese, Text::BooksLoaded) => "本已购书籍已加载",
        (Locale::SimplifiedChinese, Text::RefreshPrompt) => "请先登录再刷新书库。",
        (Locale::SimplifiedChinese, Text::Refreshing) => "正在刷新已购书籍…",
        (Locale::SimplifiedChinese, Text::RefreshFailed) => "书籍加载失败",
        (Locale::SimplifiedChinese, Text::SelectBooks) => "请至少选择一本书。",
        (Locale::SimplifiedChinese, Text::QueueRunning) => "请等待当前下载队列完成。",
        (Locale::SimplifiedChinese, Text::QueueFinished) => "下载队列已完成。",
        (Locale::SimplifiedChinese, Text::Ready) => "就绪",
        (Locale::SimplifiedChinese, Text::Queued) => "等待中",
        (Locale::SimplifiedChinese, Text::Downloading) => "下载中",
        (Locale::SimplifiedChinese, Text::Failed) => "失败",
        (Locale::SimplifiedChinese, Text::DecryptedFile) => "已导出",
        (Locale::SimplifiedChinese, Text::EncryptedFile) => "已下载",
        (Locale::SimplifiedChinese, Text::ProxySettingSaved) => "代理设置已保存。",
        (Locale::SimplifiedChinese, Text::LoginTitle) => "登录 Endao",
        (Locale::SimplifiedChinese, Text::LoginSubtitle) => "登录后访问已购书库。",
        (Locale::SimplifiedChinese, Text::LibraryNav) => "书库",
        (Locale::SimplifiedChinese, Text::DownloadsNav) => "下载",
        (Locale::SimplifiedChinese, Text::SettingsNav) => "设置",
        (Locale::SimplifiedChinese, Text::Logout) => "退出登录",
        (Locale::SimplifiedChinese, Text::Pause) => "暂停",
        (Locale::SimplifiedChinese, Text::Resume) => "继续",
        (Locale::SimplifiedChinese, Text::PauseRequested) => "已暂停，下载进度已保留。",
        (Locale::SimplifiedChinese, Text::ResumeRequested) => "已继续下载。",
        (Locale::SimplifiedChinese, Text::NoDownloads) => "暂无下载任务",
        (Locale::SimplifiedChinese, Text::NoBooks) => "未找到已购书籍",
        (Locale::SimplifiedChinese, Text::SelectedCount) => "本已选",
        (Locale::SimplifiedChinese, Text::Account) => "账户",
        (Locale::SimplifiedChinese, Text::DownloadFolder) => "下载目录",
        (Locale::SimplifiedChinese, Text::Network) => "网络",
        (Locale::SimplifiedChinese, Text::Preferences) => "偏好设置",
        (Locale::SimplifiedChinese, Text::DownloadsSubtitle) => "查看进度，暂停或继续下载。",
        (Locale::SimplifiedChinese, Text::SettingsSubtitle) => "管理保存位置、语言和网络。",
        (Locale::SimplifiedChinese, Text::SearchBooks) => "搜索书名或作者",
        (Locale::SimplifiedChinese, Text::Paused) => "已暂停",
        (Locale::SimplifiedChinese, Text::PauseAll) => "全部暂停",
        (Locale::SimplifiedChinese, Text::ResumeAll) => "全部继续",
        (Locale::SimplifiedChinese, Text::ClearCompleted) => "清除已完成",
        (Locale::SimplifiedChinese, Text::DownloadBehavior) => "下载与导出",
        (Locale::SimplifiedChinese, Text::SingleThread) => "单线程",
        (Locale::SimplifiedChinese, Text::ExperimentalParallel) => "多线程 · 实验性",
        (Locale::SimplifiedChinese, Text::DownloadModeHint) => {
            "每次下载一本书。实验性多线程会为每本书使用多个连接。"
        }
        (Locale::SimplifiedChinese, Text::NextQueueHint) => "更改将在下一批任务中生效。",
        (Locale::SimplifiedChinese, Text::KeepEncrypted) => "保留加密原件",
        (Locale::SimplifiedChinese, Text::KeepEncryptedHint) => {
            "默认关闭：成功导出后清理 .books.epub；导出失败时保留原件。"
        }
        (Locale::SimplifiedChinese, Text::Exporting) => "正在导出 EPUB…",
        (Locale::SimplifiedChinese, Text::CleanupFailed) => "已导出；加密原件清理失败",
        (Locale::SimplifiedChinese, Text::On) => "开",
        (Locale::SimplifiedChinese, Text::Off) => "关",
        (Locale::SimplifiedChinese, Text::ShowInFolder) => "打开位置",
        (Locale::SimplifiedChinese, Text::DeleteFile) => "删除文件",
        (Locale::SimplifiedChinese, Text::DeletingFile) => "正在删除文件…",
        (Locale::SimplifiedChinese, Text::MovedToTrash) => "已移至回收站",
        (Locale::SimplifiedChinese, Text::PermanentlyDeleted) => "已永久删除",
        (Locale::SimplifiedChinese, Text::FileMissing) => "文件已不存在",
        (Locale::SimplifiedChinese, Text::RevealFailed) => "无法打开文件位置",
        (Locale::SimplifiedChinese, Text::DeleteFailed) => "文件删除失败",
        (Locale::SimplifiedChinese, Text::DeletePolicy) => {
            "删除文件会将成品移至回收站；移入失败时会直接删除。"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_preferences_resolve_consistently() {
        assert_eq!(LanguagePreference::English.resolve(), Locale::English);
        assert_eq!(
            LanguagePreference::SimplifiedChinese.resolve(),
            Locale::SimplifiedChinese
        );
        assert!(!text(Locale::English, Text::SignIn).is_empty());
        assert!(!text(Locale::SimplifiedChinese, Text::SignIn).is_empty());
    }
}
