use crate::api::Book;
use std::collections::{HashMap, HashSet};

/// A portable filename component, preserving the title's typography and spaces.
fn component(value: &str) -> String {
    let value: String = value
        .chars()
        .filter(|ch| !ch.is_control())
        .map(|ch| if "<>:\"/\\|?*".contains(ch) { '_' } else { ch })
        .collect();
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let value = value.trim_matches([' ', '.']).to_string();
    let basename = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(basename.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (basename.len() == 4
            && (basename.starts_with("COM") || basename.starts_with("LPT"))
            && matches!(basename.as_bytes()[3], b'1'..=b'9'));
    if reserved { format!("_{value}") } else { value }
}

pub fn export_stem(book: &Book) -> String {
    let title = book.title();
    let mut title = title.trim();
    if title.to_ascii_lowercase().ends_with(".epub") {
        title = &title[..title.len() - 5];
    }
    let title = component(title);
    let author = component(&book.author());
    let title = if title.is_empty() {
        component(&book.id())
    } else {
        title
    };
    let mut stem = if author.is_empty() {
        title
    } else {
        format!("{title} - {author}")
    };
    // Leave room for extensions and partial-file suffixes on common filesystems.
    while stem.len() > 190 {
        stem.pop();
    }
    stem.trim_end_matches([' ', '.']).to_string()
}

/// Resolve collisions across the whole library, including on case-insensitive filesystems.
/// Book IDs disambiguate editions; no account identifier is included in filenames.
pub fn export_stems(books: &[Book]) -> HashMap<String, String> {
    let mut counts = HashMap::<String, usize>::new();
    for book in books {
        *counts.entry(export_stem(book).to_lowercase()).or_default() += 1;
    }
    let mut ordered: Vec<_> = books.iter().collect();
    ordered.sort_by_key(|book| book.id());
    let mut used = HashSet::new();
    let mut result = HashMap::new();
    for book in ordered {
        let base = export_stem(book);
        let duplicate = counts[&base.to_lowercase()] > 1;
        let mut candidate = base.clone();
        let mut counter = 0;
        while (duplicate && counter == 0) || used.contains(&candidate.to_lowercase()) {
            counter += 1;
            let suffix = if counter == 1 {
                format!(" ({})", component(&book.id()))
            } else {
                format!(" ({}-{counter})", component(&book.id()))
            };
            let mut prefix = base.clone();
            while prefix.len() + suffix.len() > 220 && !prefix.is_empty() {
                prefix.pop();
            }
            candidate = format!("{}{suffix}", prefix.trim_end());
        }
        used.insert(candidate.to_lowercase());
        result.insert(book.id(), candidate);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn book(title: &str, author: &str) -> Book {
        Book {
            raw: json!({"bookId": 7, "name": title, "author": author, "fileName": "2154.epub"}),
        }
    }

    #[test]
    fn prefers_title_and_author_over_server_filename() {
        assert_eq!(export_stem(&book("书的名字", "作者")), "书的名字 - 作者");
        assert_eq!(
            export_stem(&book("The Book", "Test Author")),
            "The Book - Test Author"
        );
    }

    #[test]
    fn sanitizes_portably_without_discarding_title_or_unicode() {
        assert_eq!(
            export_stem(&book("Title.epub", "Author/Name")),
            "Title - Author_Name"
        );
        assert_eq!(export_stem(&book("  CON  ", "")), "_CON");
        assert_eq!(
            export_stem(&book(" 书:名? . ", "作者\\译者")),
            "书_名_ - 作者_译者"
        );
        let long = export_stem(&book(&"书".repeat(200), "作者"));
        assert!(long.len() <= 190);
    }

    #[test]
    fn duplicate_titles_do_not_overwrite_other_editions() {
        let first = book("Book", "Author");
        let mut second = book("book", "Author");
        second.raw["bookId"] = json!(8);
        let names = export_stems(&[first.clone(), second.clone()]);
        assert_eq!(names["7"], "Book - Author (7)");
        assert_eq!(names["8"], "book - Author (8)");
        assert_eq!(names, export_stems(&[second, first]));
    }
}
