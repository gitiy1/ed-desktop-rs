use super::*;
use serde_json::json;

impl AppView {
    pub(super) fn demo(page: &str, cx: &mut Context<Self>) -> Self {
        let mut view = Self::new(cx);
        view.demo = true;
        view.persist_settings = false;
        view.email.clear();
        view.password.clear();
        view.proxy_address.clear();
        view.workers_text = "4".to_string();
        view.settings = AppSettings {
            language: LanguagePreference::English,
            ..AppSettings::default()
        };
        view.output_text = "~/Downloads/Endao Books".to_string();
        view.session = Some(Session {
            email: "reader@example.test".to_string(),
            access_token: String::new(),
            token: String::new(),
            device_id: "demo-device".to_string(),
            device_name: "Desktop Demo".to_string(),
            device_type: "Desktop".to_string(),
            version: "demo".to_string(),
            api_base: String::new(),
        });
        view.rows = demo_books()
            .into_iter()
            .map(|book| BookRow::new(book, Locale::English))
            .collect();
        view.status = "Demo data · no network requests".to_string();
        view.page = match page.to_ascii_lowercase().as_str() {
            "downloads" => ActivePage::Downloads,
            "settings" => ActivePage::Settings,
            "login" => {
                view.session = None;
                view.rows.clear();
                ActivePage::Library
            }
            _ => ActivePage::Library,
        };
        if view.page == ActivePage::Downloads {
            view.queued_download_ids = view.rows.iter().take(3).map(|row| row.book.id()).collect();
            view.rows[0].state = DownloadState::Paused;
            view.rows[0].received = 1_920_000;
            view.rows[0].total = 4_800_000;
            view.rows[0].speed = 742_000.0;
            view.rows[1].state = DownloadState::Queued;
            view.rows[1].received = 2_300_000;
            view.rows[1].total = 3_600_000;
            let control = Arc::new(DownloadControl::default());
            control.pause();
            view.download_control = Some(control);
            view.busy = true;
        }
        view
    }

    pub(super) fn load_demo_library(&mut self) {
        self.password.clear();
        self.focused_field = FocusedField::Search;
        self.cursor = 0;
        self.select_all = false;
        self.session = Some(Session {
            email: "reader@example.test".to_string(),
            access_token: String::new(),
            token: String::new(),
            device_id: "demo-device".to_string(),
            device_name: "Desktop Demo".to_string(),
            device_type: "Desktop".to_string(),
            version: "demo".to_string(),
            api_base: String::new(),
        });
        self.rows = demo_books()
            .into_iter()
            .map(|book| BookRow::new(book, self.locale()))
            .collect();
        self.status = "Demo data · no network requests".to_string();
    }

    pub(super) fn start_demo_downloads(&mut self, cx: &mut Context<Self>) {
        let selected: Vec<_> = self
            .rows
            .iter_mut()
            .filter(|row| row.selected)
            .map(|row| {
                row.state = DownloadState::Queued;
                row.total = 0;
                row.received = 0;
                row.book.id()
            })
            .collect();
        if selected.is_empty() {
            self.status = self.text(Text::SelectBooks).to_string();
        } else {
            if let Some(row) = self.rows.iter_mut().find(|row| row.selected) {
                row.state = DownloadState::Downloading;
                row.total = 4_800_000;
                row.received = 1_920_000;
                row.speed = 742_000.0;
            }
            self.queued_download_ids.extend(selected);
            self.busy = true;
            self.download_control = Some(Arc::new(DownloadControl::default()));
            self.page = ActivePage::Downloads;
            self.status = "Demo data · no network requests".to_string();
        }
        cx.notify();
    }
}

fn demo_books() -> Vec<Book> {
    [
        ("The Garden of Small Hours", "Mira Ellison"),
        ("A Map of Quiet Places", "Jonah Reed"),
        ("Letters from the Coast", "Lin Qiao"),
        ("The Last Paper Crane", "Amelia Hart"),
        ("After the Rain", "S. Nakamura"),
        ("A Lantern in Winter", "Mira Ellison"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (title, author))| Book {
        raw: json!({
            "bookId": format!("demo-{}", index + 1),
            "name": title,
            "author": author,
        }),
    })
    .collect()
}
