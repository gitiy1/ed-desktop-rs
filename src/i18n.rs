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
    AppSubtitle,
    SignedIn,
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
    FollowSystem,
    English,
    SimplifiedChinese,
    Proxy,
    SystemProxy,
    ManualProxy,
    HttpProxy,
    Socks5Proxy,
    ProxyAddress,
    ProxyAddressPlaceholder,
    SystemProxyFallback,
    PurchasedBooks,
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
    Downloaded,
    Failed,
    DecryptedFile,
    EncryptedFile,
    ProxySettingSaved,
}

pub fn text(locale: Locale, key: Text) -> &'static str {
    match (locale, key) {
        (Locale::English, Text::AppSubtitle) => "Purchased library downloader",
        (Locale::English, Text::SignedIn) => "Signed in",
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
        (Locale::English, Text::FollowSystem) => "System",
        (Locale::English, Text::English) => "English",
        (Locale::English, Text::SimplifiedChinese) => "简体中文",
        (Locale::English, Text::Proxy) => "PROXY",
        (Locale::English, Text::SystemProxy) => "System",
        (Locale::English, Text::ManualProxy) => "Manual",
        (Locale::English, Text::HttpProxy) => "HTTP",
        (Locale::English, Text::Socks5Proxy) => "SOCKS5",
        (Locale::English, Text::ProxyAddress) => "ADDRESS",
        (Locale::English, Text::ProxyAddressPlaceholder) => "host:port",
        (Locale::English, Text::SystemProxyFallback) => {
            "Uses OS proxy settings, then proxy environment variables"
        }
        (Locale::English, Text::PurchasedBooks) => "PURCHASED BOOKS",
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
        (Locale::English, Text::Downloaded) => "Downloaded",
        (Locale::English, Text::Failed) => "Failed",
        (Locale::English, Text::DecryptedFile) => "Ready",
        (Locale::English, Text::EncryptedFile) => "Downloaded",
        (Locale::English, Text::ProxySettingSaved) => "Proxy settings saved.",
        (Locale::SimplifiedChinese, Text::AppSubtitle) => "已购书籍下载",
        (Locale::SimplifiedChinese, Text::SignedIn) => "已登录",
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
        (Locale::SimplifiedChinese, Text::FollowSystem) => "跟随系统",
        (Locale::SimplifiedChinese, Text::English) => "English",
        (Locale::SimplifiedChinese, Text::SimplifiedChinese) => "简体中文",
        (Locale::SimplifiedChinese, Text::Proxy) => "代理",
        (Locale::SimplifiedChinese, Text::SystemProxy) => "系统",
        (Locale::SimplifiedChinese, Text::ManualProxy) => "手动",
        (Locale::SimplifiedChinese, Text::HttpProxy) => "HTTP",
        (Locale::SimplifiedChinese, Text::Socks5Proxy) => "SOCKS5",
        (Locale::SimplifiedChinese, Text::ProxyAddress) => "地址",
        (Locale::SimplifiedChinese, Text::ProxyAddressPlaceholder) => "主机:端口",
        (Locale::SimplifiedChinese, Text::SystemProxyFallback) => {
            "读取系统代理；不可用时使用代理环境变量"
        }
        (Locale::SimplifiedChinese, Text::PurchasedBooks) => "已购书籍",
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
        (Locale::SimplifiedChinese, Text::Downloaded) => "已下载",
        (Locale::SimplifiedChinese, Text::Failed) => "失败",
        (Locale::SimplifiedChinese, Text::DecryptedFile) => "已导出",
        (Locale::SimplifiedChinese, Text::EncryptedFile) => "已下载",
        (Locale::SimplifiedChinese, Text::ProxySettingSaved) => "代理设置已保存。",
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
