use crate::api::{Book, Session, old_download_request};
use crate::network;
use crate::settings::ProxySettings;
use anyhow::{Context, Result, anyhow};
use reqwest::StatusCode;
use reqwest::header::{CONTENT_RANGE, RANGE};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct DownloadProgress {
    pub received: u64,
    pub total: u64,
    pub bytes_per_second: f64,
}

type ProgressCallback = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

#[derive(Default)]
pub struct DownloadControl {
    paused: Mutex<bool>,
    resumed: Condvar,
    cancelled: AtomicBool,
}

#[derive(Debug)]
struct DownloadCancelled;

impl std::fmt::Display for DownloadCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Download stopped; partial files retained")
    }
}

impl std::error::Error for DownloadCancelled {}

#[derive(Debug)]
struct TransportReadError(std::io::Error);

impl std::fmt::Display for TransportReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for TransportReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl DownloadControl {
    pub fn pause(&self) {
        *self.paused.lock().expect("download control poisoned") = true;
    }

    pub fn resume(&self) {
        *self.paused.lock().expect("download control poisoned") = false;
        self.resumed.notify_all();
    }

    pub fn is_paused(&self) -> bool {
        *self.paused.lock().expect("download control poisoned")
    }

    pub fn cancel(&self) {
        let _paused = self.paused.lock().expect("download control poisoned");
        self.cancelled.store(true, Ordering::Release);
        self.resumed.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn wait_until_resumed(&self) -> Result<()> {
        let mut paused = self.paused.lock().expect("download control poisoned");
        while *paused && !self.is_cancelled() {
            paused = self
                .resumed
                .wait(paused)
                .expect("download control poisoned");
        }
        if self.is_cancelled() {
            return Err(DownloadCancelled.into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RangeMeta {
    total: u64,
    ranges: Vec<(u64, u64)>,
}

#[derive(Clone, Debug)]
struct RangeJob {
    start: u64,
    end: u64,
    path: PathBuf,
}

struct RangeOptions {
    total: u64,
    workers: usize,
}

struct Reporter {
    received: AtomicU64,
    total: AtomicU64,
    last_sent: Mutex<(Instant, u64)>,
    callback: ProgressCallback,
}

impl Reporter {
    fn new(initial: u64, total: u64, callback: ProgressCallback) -> Self {
        Self {
            received: AtomicU64::new(initial),
            total: AtomicU64::new(total),
            last_sent: Mutex::new((Instant::now() - Duration::from_secs(1), initial)),
            callback,
        }
    }

    fn add(&self, amount: u64) {
        let received = self.received.fetch_add(amount, Ordering::Relaxed) + amount;
        let mut last_sent = self.last_sent.lock().expect("progress timestamp poisoned");
        let total = self.total.load(Ordering::Relaxed);
        if last_sent.0.elapsed() >= Duration::from_millis(120) || received >= total {
            let elapsed = last_sent.0.elapsed().as_secs_f64().max(0.001);
            (self.callback)(DownloadProgress {
                received,
                total,
                bytes_per_second: received.saturating_sub(last_sent.1) as f64 / elapsed,
            });
            *last_sent = (Instant::now(), received);
        }
    }
}

pub fn download_legacy(
    session: &Session,
    book: &Book,
    destination: &Path,
    workers: usize,
    proxy: &ProxySettings,
    control: Arc<DownloadControl>,
    callback: impl Fn(DownloadProgress) + Send + Sync + 'static,
) -> Result<PathBuf> {
    if book.id() == "?" {
        return Err(anyhow!("Book has no book ID."));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let client = network::http_client(Duration::from_secs(90), "okhttp/4.12.0", proxy)?;
    let callback: ProgressCallback = Arc::new(callback);
    let partial = append_suffix(destination, ".part");
    control.wait_until_resumed()?;

    if workers <= 1 {
        let result = download_sequential(
            &client,
            session,
            book,
            destination,
            &partial,
            control,
            callback,
        );
        if result.is_ok() {
            let _ = fs::remove_dir_all(append_suffix(destination, ".parts"));
        }
        return result;
    }

    let probe = old_download_request(&client, session, book)
        .header(RANGE, "bytes=0-0")
        .send()
        .context("probe legacy download range support")?;

    if probe.status() == StatusCode::PARTIAL_CONTENT {
        let (start, end, total) = parse_content_range(probe.headers().get(CONTENT_RANGE))
            .ok_or_else(|| anyhow!("Range probe did not include a valid Content-Range."))?;
        if start != 0 || end != 0 {
            return Err(anyhow!("Range probe returned an unexpected byte range."));
        }
        drop(probe);
        match download_ranged(
            &client,
            session,
            book,
            destination,
            RangeOptions { total, workers },
            control.clone(),
            Arc::clone(&callback),
        ) {
            Ok(path) => Ok(path),
            Err(range_error) => {
                control.wait_until_resumed()?;
                match download_sequential(
                    &client,
                    session,
                    book,
                    destination,
                    &partial,
                    control,
                    callback,
                ) {
                    Ok(path) => {
                        let _ = fs::remove_dir_all(append_suffix(destination, ".parts"));
                        Ok(path)
                    }
                    Err(sequential_error) => Err(anyhow!(
                        "Parallel download failed ({range_error:#}); sequential fallback failed ({sequential_error:#})."
                    )),
                }
            }
        }
    } else if probe.status().is_success() {
        let total = probe.content_length().unwrap_or(0);
        let result = stream_response(
            probe,
            destination,
            &partial,
            false,
            total,
            control,
            callback,
        );
        if result.is_ok() {
            let _ = fs::remove_dir_all(append_suffix(destination, ".parts"));
        }
        result
    } else {
        let status = probe.status();
        let message = probe.text().unwrap_or_default();
        Err(anyhow!(
            "Legacy download returned HTTP {status}: {}",
            message.chars().take(500).collect::<String>()
        ))
    }
}

fn download_sequential(
    client: &reqwest::blocking::Client,
    session: &Session,
    book: &Book,
    destination: &Path,
    partial: &Path,
    control: Arc<DownloadControl>,
    callback: ProgressCallback,
) -> Result<PathBuf> {
    let mut last_error = None;
    for attempt in 0..3 {
        control.wait_until_resumed()?;
        match download_sequential_once(
            client,
            session,
            book,
            destination,
            partial,
            Arc::clone(&control),
            Arc::clone(&callback),
        ) {
            Ok(path) => return Ok(path),
            Err(error) if is_network_error(&error) && attempt < 2 => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(250 * (attempt + 1) as u64));
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("Legacy download failed after retries.")))
}

fn download_sequential_once(
    client: &reqwest::blocking::Client,
    session: &Session,
    book: &Book,
    destination: &Path,
    partial: &Path,
    control: Arc<DownloadControl>,
    callback: ProgressCallback,
) -> Result<PathBuf> {
    control.wait_until_resumed()?;
    let offset = fs::metadata(partial)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let mut request = old_download_request(client, session, book);
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    let response = request.send().context("request legacy book download")?;
    if response.status() == StatusCode::RANGE_NOT_SATISFIABLE && offset > 0 {
        let completed_total = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("bytes */"))
            .and_then(|value| value.parse::<u64>().ok());
        if completed_total == Some(offset) {
            control.wait_until_resumed()?;
            replace_file(partial, destination)?;
            callback(DownloadProgress {
                received: offset,
                total: offset,
                bytes_per_second: 0.0,
            });
            return Ok(destination.to_path_buf());
        }
    }
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().unwrap_or_default();
        return Err(anyhow!(
            "Legacy download returned HTTP {status}: {}",
            message.chars().take(500).collect::<String>()
        ));
    }
    let append = offset > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
    if append {
        let range = parse_content_range(response.headers().get(CONTENT_RANGE));
        if !range.is_some_and(|(start, _, _)| start == offset) {
            return Err(anyhow!(
                "Resume response did not start at the requested byte offset."
            ));
        }
    }
    let total = parse_content_range_total(response.headers().get(CONTENT_RANGE))
        .or_else(|| {
            response
                .content_length()
                .map(|length| length + if append { offset } else { 0 })
        })
        .unwrap_or(0);
    stream_response(
        response,
        destination,
        partial,
        append,
        total,
        control,
        callback,
    )
}

fn is_network_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<reqwest::Error>().is_some()
            || cause.downcast_ref::<TransportReadError>().is_some()
    })
}

fn stream_response(
    mut response: reqwest::blocking::Response,
    destination: &Path,
    partial: &Path,
    append: bool,
    total: u64,
    control: Arc<DownloadControl>,
    callback: ProgressCallback,
) -> Result<PathBuf> {
    let existing = if append {
        fs::metadata(partial)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    } else {
        0
    };
    let reporter = Reporter::new(existing, total, callback);
    let mut output = OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(partial)
        .with_context(|| format!("open {}", partial.display()))?;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = response
            .read(&mut buffer)
            .map_err(TransportReadError)
            .context("read legacy download stream")?;
        if count == 0 {
            break;
        }
        control.wait_until_resumed()?;
        output
            .write_all(&buffer[..count])
            .context("write partial EPUB")?;
        reporter.add(count as u64);
        control.wait_until_resumed()?;
    }
    output.flush().context("flush downloaded EPUB")?;
    output.sync_all().context("sync downloaded EPUB")?;
    let downloaded_size = output.metadata().context("inspect downloaded EPUB")?.len();
    if total > 0 && downloaded_size != total {
        return Err(anyhow!(
            "Download ended early: {downloaded_size}/{total} bytes. The partial file is kept for resume."
        ));
    }
    control.wait_until_resumed()?;
    drop(output);
    replace_file(partial, destination)?;
    Ok(destination.to_path_buf())
}

fn download_ranged(
    client: &reqwest::blocking::Client,
    session: &Session,
    book: &Book,
    destination: &Path,
    options: RangeOptions,
    control: Arc<DownloadControl>,
    callback: ProgressCallback,
) -> Result<PathBuf> {
    let RangeOptions { total, workers } = options;
    if total == 0 {
        return Err(anyhow!("Server returned an empty book."));
    }
    let parts_dir = append_suffix(destination, ".parts");
    fs::create_dir_all(&parts_dir).with_context(|| format!("create {}", parts_dir.display()))?;
    let map_path = parts_dir.join("ranges.json");
    let ranges = fs::read(&map_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RangeMeta>(&bytes).ok())
        .filter(|meta| meta.total == total && !meta.ranges.is_empty())
        .map(|meta| meta.ranges)
        .unwrap_or_else(|| split_ranges(total, workers));
    fs::write(
        &map_path,
        serde_json::to_vec(&RangeMeta {
            total,
            ranges: ranges.clone(),
        })?,
    )
    .context("write range map")?;

    let mut jobs: Vec<RangeJob> = ranges
        .into_iter()
        .enumerate()
        .map(|(index, (start, end))| RangeJob {
            start,
            end,
            path: parts_dir.join(format!("{:05}-{start}-{end}.part", index)),
        })
        .collect();

    let mut initial = 0;
    for job in &jobs {
        control.wait_until_resumed()?;
        let expected = job.end - job.start + 1;
        if let Ok(metadata) = fs::metadata(&job.path) {
            initial += metadata.len().min(expected);
        }
    }
    let reporter = Arc::new(Reporter::new(initial, total, callback));
    let queue = Arc::new(Mutex::new(VecDeque::from(
        jobs.iter()
            .filter(|job| {
                let expected = job.end - job.start + 1;
                !fs::metadata(&job.path).is_ok_and(|metadata| metadata.len() == expected)
            })
            .cloned()
            .collect::<Vec<_>>(),
    )));
    let failed = Arc::new(AtomicBool::new(false));
    let error = Arc::new(Mutex::new(None::<String>));
    let worker_count = workers
        .max(1)
        .min(queue.lock().expect("range queue poisoned").len().max(1));

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let client = client.clone();
            let queue = Arc::clone(&queue);
            let failed = Arc::clone(&failed);
            let error = Arc::clone(&error);
            let reporter = Arc::clone(&reporter);
            let control = Arc::clone(&control);
            scope.spawn(move || {
                loop {
                    if failed.load(Ordering::Acquire) {
                        break;
                    }
                    let job = queue.lock().expect("range queue poisoned").pop_front();
                    let Some(job) = job else { break };
                    if let Err(problem) =
                        fetch_range(&client, session, book, &job, &reporter, &control)
                    {
                        failed.store(true, Ordering::Release);
                        *error.lock().expect("range error poisoned") = Some(problem.to_string());
                        break;
                    }
                }
            });
        }
    });

    if let Some(problem) = error.lock().expect("range error poisoned").take() {
        return Err(anyhow!("Parallel range download failed: {problem}"));
    }

    let mut covered_total = total;
    let mut actual_total = reporter.total.load(Ordering::Acquire);
    while actual_total > covered_total {
        let tail = RangeJob {
            start: covered_total,
            end: actual_total - 1,
            path: parts_dir.join(format!(
                "{:05}-{covered_total}-{}.part",
                jobs.len(),
                actual_total - 1
            )),
        };
        fetch_range(client, session, book, &tail, &reporter, &control)?;
        jobs.push(tail);
        covered_total = actual_total;
        actual_total = reporter.total.load(Ordering::Acquire);
    }

    let partial = append_suffix(destination, ".part");
    let mut output = BufWriter::new(File::create(&partial).context("create merged EPUB")?);
    for job in &jobs {
        control.wait_until_resumed()?;
        let expected = job.end - job.start + 1;
        let actual = fs::metadata(&job.path)
            .with_context(|| format!("missing range {}-{}", job.start, job.end))?
            .len();
        if actual != expected {
            return Err(anyhow!(
                "Range {}-{} is incomplete ({actual}/{expected} bytes).",
                job.start,
                job.end
            ));
        }
        let mut input = BufReader::new(File::open(&job.path)?);
        std::io::copy(&mut input, &mut output).context("merge EPUB ranges")?;
    }
    output.flush().context("flush merged EPUB")?;
    output.get_ref().sync_all().context("sync merged EPUB")?;
    let merged_size = output
        .get_ref()
        .metadata()
        .context("inspect merged EPUB")?
        .len();
    if merged_size != actual_total {
        return Err(anyhow!(
            "Merged file has {merged_size} bytes, expected {actual_total}. Range parts have been retained for resume."
        ));
    }
    control.wait_until_resumed()?;
    drop(output);
    replace_file(&partial, destination)?;
    let _ = fs::remove_dir_all(&parts_dir);
    Ok(destination.to_path_buf())
}

fn fetch_range(
    client: &reqwest::blocking::Client,
    session: &Session,
    book: &Book,
    job: &RangeJob,
    reporter: &Reporter,
    control: &DownloadControl,
) -> Result<()> {
    for attempt in 0..3 {
        control.wait_until_resumed()?;
        match fetch_range_once(client, session, book, job, reporter, control) {
            Ok(()) => return Ok(()),
            Err(error) if is_network_error(&error) && attempt < 2 => {
                thread::sleep(Duration::from_millis(250 * (attempt + 1)));
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("last range attempt returns its error")
}

fn fetch_range_once(
    client: &reqwest::blocking::Client,
    session: &Session,
    book: &Book,
    job: &RangeJob,
    reporter: &Reporter,
    control: &DownloadControl,
) -> Result<()> {
    let expected = job.end - job.start + 1;
    let mut offset = fs::metadata(&job.path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if offset == expected {
        return Ok(());
    }
    if offset > expected {
        fs::remove_file(&job.path).context("remove an oversized range part")?;
        offset = 0;
    }
    let start = job.start + offset;
    control.wait_until_resumed()?;
    let response = old_download_request(client, session, book)
        .header(RANGE, format!("bytes={start}-{}", job.end))
        .send()
        .context("request range")?;
    if response.status() != StatusCode::PARTIAL_CONTENT {
        return Err(anyhow!(
            "server stopped honoring Range (HTTP {})",
            response.status()
        ));
    }
    let Some((range_start, range_end, server_total)) =
        parse_content_range(response.headers().get(CONTENT_RANGE))
    else {
        return Err(anyhow!(
            "Range response did not include a valid Content-Range."
        ));
    };
    if range_start != start || range_end != job.end || server_total <= range_end {
        return Err(anyhow!(
            "server returned bytes {range_start}-{range_end}/{server_total}, expected {start}-{}",
            job.end
        ));
    }
    reporter.total.fetch_max(server_total, Ordering::AcqRel);
    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&job.path)
        .context("open range part")?;
    let mut response = response;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = response
            .read(&mut buffer)
            .map_err(TransportReadError)
            .context("read range response")?;
        if count == 0 {
            break;
        }
        control.wait_until_resumed()?;
        output
            .write_all(&buffer[..count])
            .context("write range part")?;
        reporter.add(count as u64);
        control.wait_until_resumed()?;
    }
    output.flush().context("flush range part")?;
    output.sync_all().context("sync range part")?;
    let actual = output.metadata().context("inspect open range part")?.len();
    if actual != expected {
        return Err(anyhow!(
            "range {}-{} returned {actual}/{expected} bytes",
            job.start,
            job.end
        ));
    }
    Ok(())
}

fn split_ranges(total: u64, workers: usize) -> Vec<(u64, u64)> {
    let count = workers.max(1).min(total as usize);
    let span = total.div_ceil(count as u64);
    let mut ranges = Vec::with_capacity(count);
    let mut start = 0;
    while start < total {
        let end = (start + span - 1).min(total - 1);
        ranges.push((start, end));
        start = end + 1;
    }
    ranges
}

fn parse_content_range_total(value: Option<&reqwest::header::HeaderValue>) -> Option<u64> {
    parse_content_range(value).map(|(_, _, total)| total)
}

fn parse_content_range(value: Option<&reqwest::header::HeaderValue>) -> Option<(u64, u64, u64)> {
    let value = value?.to_str().ok()?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.strip_prefix("bytes ")?.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?, total.parse().ok()?))
}

fn replace_file(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(windows)]
    if destination.exists() {
        fs::remove_file(destination)
            .with_context(|| format!("replace existing file {}", destination.display()))?;
    }
    fs::rename(source, destination)
        .with_context(|| format!("move downloaded file to {}", destination.display()))?;
    Ok(())
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread::JoinHandle;

    fn make_session(api_base: String) -> Session {
        Session {
            email: "reader@example.com".to_string(),
            access_token: "access".to_string(),
            token: "token".to_string(),
            device_id: "0123456789abcdef".to_string(),
            device_name: "Android Device".to_string(),
            device_type: "Android SDK_35".to_string(),
            version: "2.5.0".to_string(),
            api_base,
        }
    }

    fn start_range_server(
        listener: TcpListener,
        content: Vec<u8>,
        request_count: usize,
        probe_total: Option<usize>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            for _ in 0..request_count {
                let (mut stream, _) = listener.accept().unwrap();
                let range = read_request(&mut stream);
                let (start, end) = match range
                    .as_deref()
                    .and_then(|range| range.strip_prefix("bytes="))
                {
                    Some(range) => {
                        let (start, end) = range.split_once('-').unwrap();
                        let start: usize = start.parse().unwrap();
                        let end = if end.is_empty() {
                            content.len() - 1
                        } else {
                            end.parse().unwrap()
                        };
                        (start, end)
                    }
                    None => (0, content.len() - 1),
                };
                let body = &content[start..=end];
                let advertised_total = if range.as_deref() == Some("bytes=0-0") {
                    probe_total.unwrap_or(content.len())
                } else {
                    content.len()
                };
                let response = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    advertised_total,
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.write_all(body).unwrap();
                stream.flush().unwrap();
            }
        })
    }

    fn read_request(stream: &mut TcpStream) -> Option<String> {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let count = stream.read(&mut buffer).unwrap();
            assert_ne!(count, 0, "client closed before completing its request");
            request.extend_from_slice(&buffer[..count]);
            if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let header_end = position + 4;
                let header_text = String::from_utf8_lossy(&request[..position]);
                let content_length = header_text
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + content_length {
                    let range = header_text.lines().find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("range")
                            .then(|| value.trim().to_string())
                    });
                    return range;
                }
            }
        }
    }

    fn test_book() -> Book {
        Book {
            raw: json!({ "bookId": "test-book", "name": "Sample" }),
        }
    }

    #[test]
    fn split_ranges_are_contiguous_and_cover_the_file() {
        let ranges = split_ranges(10, 4);
        assert_eq!(ranges, vec![(0, 2), (3, 5), (6, 8), (9, 9)]);
        assert_eq!(ranges.first().map(|range| range.0), Some(0));
        assert_eq!(ranges.last().map(|range| range.1), Some(9));
        assert!(ranges.windows(2).all(|pair| pair[0].1 + 1 == pair[1].0));
    }

    #[test]
    fn download_control_blocks_workers_until_resume() {
        let control = Arc::new(DownloadControl::default());
        control.pause();
        let worker_control = Arc::clone(&control);
        let (sent, received) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            worker_control.wait_until_resumed().unwrap();
            sent.send(()).unwrap();
        });
        assert!(received.recv_timeout(Duration::from_millis(30)).is_err());
        control.resume();
        received.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap();
    }

    fn partial_bytes(output: &Path) -> u64 {
        fn readable_bytes(path: &Path) -> u64 {
            // Windows can retain a stale directory-entry size while the writer
            // is open. Count bytes actually readable from the file instead.
            match File::open(path) {
                Ok(mut file) => std::io::copy(&mut file, &mut std::io::sink())
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
                Err(error) => panic!("open {}: {error}", path.display()),
            }
        }

        let mut received = readable_bytes(&append_suffix(output, ".part"));
        match fs::read_dir(append_suffix(output, ".parts")) {
            Ok(entries) => {
                for entry in entries {
                    let path = entry.expect("read range directory entry").path();
                    if path.extension().is_some_and(|ext| ext == "part") {
                        received += readable_bytes(&path);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("read range directory for {}: {error}", output.display()),
        }
        received
    }

    #[test]
    fn real_http_stream_and_parallel_workers_pause_then_resume_without_data_loss() {
        for workers in [1, 3] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let content: Vec<u8> = (0..512 * 1024).map(|index| (index % 239) as u8).collect();
            let request_count = if workers == 1 { 1 } else { workers + 1 };
            let server = start_range_server(listener, content.clone(), request_count, None);
            let directory = tempfile::tempdir().unwrap();
            let output = directory.path().join("pause.epub");
            let session = make_session(format!("http://{address}/api/v4/"));
            let control = Arc::new(DownloadControl::default());
            let (paused_tx, paused_rx) = std::sync::mpsc::channel();
            let (done_tx, done_rx) = std::sync::mpsc::channel();
            let thread_control = control.clone();
            let thread_output = output.clone();
            let worker = thread::spawn(move || {
                let did_pause = AtomicBool::new(false);
                let callback_control = thread_control.clone();
                let result = download_legacy(
                    &session,
                    &test_book(),
                    &thread_output,
                    workers,
                    &ProxySettings::default(),
                    thread_control,
                    move |_| {
                        if !did_pause.swap(true, Ordering::AcqRel) {
                            callback_control.pause();
                            paused_tx.send(()).unwrap();
                        }
                    },
                );
                done_tx.send(result).unwrap();
            });
            paused_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(done_rx.recv_timeout(Duration::from_millis(30)).is_err());
            let paused_size = partial_bytes(&output);
            assert!(
                paused_size > 0 && paused_size < content.len() as u64,
                "{workers} workers paused with {paused_size}/{} readable bytes",
                content.len()
            );
            assert!(done_rx.recv_timeout(Duration::from_millis(30)).is_err());
            assert_eq!(
                partial_bytes(&output),
                paused_size,
                "paused workers kept writing"
            );
            control.resume();
            done_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            worker.join().unwrap();
            server.join().unwrap();
            assert_eq!(fs::read(&output).unwrap(), content);
        }
    }

    #[test]
    fn cancelling_a_paused_download_retains_partial_for_a_later_session() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let content: Vec<u8> = (0..512 * 1024).map(|index| (index % 241) as u8).collect();
        let server = start_range_server(listener, content.clone(), 2, None);
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("cancel.epub");
        let session = make_session(format!("http://{address}/api/v4/"));
        let control = Arc::new(DownloadControl::default());
        let (paused_tx, paused_rx) = std::sync::mpsc::channel();
        let thread_control = control.clone();
        let thread_output = output.clone();
        let thread_session = session.clone();
        let worker = thread::spawn(move || {
            let callback_control = thread_control.clone();
            download_legacy(
                &thread_session,
                &test_book(),
                &thread_output,
                1,
                &ProxySettings::default(),
                thread_control,
                move |_| {
                    callback_control.pause();
                    paused_tx.send(()).unwrap();
                },
            )
        });
        paused_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        control.cancel();
        assert!(worker.join().unwrap().is_err());
        assert!(partial_bytes(&output) > 0);
        assert!(!output.exists());
        download_legacy(
            &session,
            &test_book(),
            &output,
            1,
            &ProxySettings::default(),
            Arc::new(DownloadControl::default()),
            |_| {},
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(output).unwrap(), content);
    }

    #[test]
    fn a_range_reconnects_at_its_saved_offset_if_the_server_closes_while_paused() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let content: Vec<u8> = (0..256 * 1024).map(|index| (index % 227) as u8).collect();
        let served = content.clone();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            assert_eq!(
                read_request(&mut stream).unwrap(),
                format!("bytes=0-{}", served.len() - 1)
            );
            let header = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                served.len() - 1,
                served.len(),
                served.len()
            );
            stream.write_all(header.as_bytes()).unwrap();
            stream.write_all(&served[..32768]).unwrap();
            stream.flush().unwrap();
            drop(stream);
            let (mut stream, _) = listener.accept().unwrap();
            assert_eq!(
                read_request(&mut stream).unwrap(),
                format!("bytes=32768-{}", served.len() - 1)
            );
            let header = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 32768-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                served.len() - 1,
                served.len(),
                served.len() - 32768
            );
            stream.write_all(header.as_bytes()).unwrap();
            stream.write_all(&served[32768..]).unwrap();
        });
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("reconnect.epub");
        let session = make_session(format!("http://{address}/api/v4/"));
        let control = Arc::new(DownloadControl::default());
        let (paused_tx, paused_rx) = std::sync::mpsc::channel();
        let thread_control = control.clone();
        let thread_output = output.clone();
        let worker = thread::spawn(move || {
            let client =
                network::http_client(Duration::from_secs(5), "test", &ProxySettings::default())
                    .unwrap();
            let callback_control = thread_control.clone();
            let paused = AtomicBool::new(false);
            download_ranged(
                &client,
                &session,
                &test_book(),
                &thread_output,
                RangeOptions {
                    total: 256 * 1024,
                    workers: 1,
                },
                thread_control,
                Arc::new(move |_| {
                    if !paused.swap(true, Ordering::AcqRel) {
                        callback_control.pause();
                        paused_tx.send(()).unwrap();
                    }
                }),
            )
        });
        paused_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        control.resume();
        worker.join().unwrap().unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(output).unwrap(), content);
    }

    #[test]
    fn small_files_use_no_more_ranges_than_bytes() {
        assert_eq!(split_ranges(2, 8), vec![(0, 0), (1, 1)]);
        assert_eq!(split_ranges(1, 4), vec![(0, 0)]);
    }

    #[test]
    fn content_range_parser_reads_start_end_and_total() {
        let header = reqwest::header::HeaderValue::from_static("bytes 12-19/100");
        assert_eq!(parse_content_range(Some(&header)), Some((12, 19, 100)));
    }

    #[test]
    fn old_post_download_fetches_and_merges_parallel_ranges() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let content: Vec<u8> = (0..512 * 1024).map(|index| (index % 251) as u8).collect();
        let server = start_range_server(listener, content.clone(), 4, None);
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("parallel.epub");
        let session = make_session(format!("http://{address}/api/v4/"));
        let downloaded = download_legacy(
            &session,
            &test_book(),
            &output,
            3,
            &ProxySettings::default(),
            Arc::new(DownloadControl::default()),
            |_| {},
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(downloaded).unwrap(), content);
        assert!(!append_suffix(&output, ".parts").exists());
    }

    #[test]
    fn sequential_download_resumes_from_existing_partial_file() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let content: Vec<u8> = (0..128 * 1024).map(|index| (index % 239) as u8).collect();
        let server = start_range_server(listener, content.clone(), 1, None);
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("resume.epub");
        let partial = append_suffix(&output, ".part");
        let completed = 8192;
        fs::write(&partial, &content[..completed]).unwrap();
        let session = make_session(format!("http://{address}/api/v4/"));
        let downloaded = download_legacy(
            &session,
            &test_book(),
            &output,
            1,
            &ProxySettings::default(),
            Arc::new(DownloadControl::default()),
            |_| {},
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(downloaded).unwrap(), content);
        assert!(!partial.exists());
    }

    #[test]
    fn sequential_resume_recognizes_a_fully_received_partial() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            assert_eq!(read_request(&mut stream).as_deref(), Some("bytes=4-"));
            stream.write_all(b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */4\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("complete.epub");
        fs::write(append_suffix(&output, ".part"), b"book").unwrap();
        download_legacy(
            &make_session(format!("http://{address}/api/v4/")),
            &test_book(),
            &output,
            1,
            &ProxySettings::default(),
            Arc::new(DownloadControl::default()),
            |_| {},
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(output).unwrap(), b"book");
    }

    #[test]
    fn parallel_download_recovers_a_stale_probe_length() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let content: Vec<u8> = (0..512 * 1024).map(|index| (index % 223) as u8).collect();
        let server = start_range_server(listener, content.clone(), 5, Some(content.len() - 3));
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("variable-size.epub");
        let session = make_session(format!("http://{address}/api/v4/"));
        let downloaded = download_legacy(
            &session,
            &test_book(),
            &output,
            3,
            &ProxySettings::default(),
            Arc::new(DownloadControl::default()),
            |_| {},
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(downloaded).unwrap(), content);
    }
}
