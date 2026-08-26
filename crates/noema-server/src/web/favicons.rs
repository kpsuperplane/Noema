//! On-demand public-site favicon retrieval and rebuildable local caching.

use std::{
    collections::HashMap,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Arc, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use image::{
    DynamicImage, GenericImageView, ImageFormat, ImageReader, Limits, RgbaImage, imageops,
};
use noema_home::{atomic_write_private, ensure_private_dir};
use noema_providers::{WebFetchError, checked_public_http_client, validate_public_url_parsed};
use reqwest::header;
use ring::digest;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Semaphore};
use url::{Host, Url};

const CACHE_LIMIT: usize = 1_024;
const ICON_BYTES_LIMIT: usize = 256 * 1024;
const HTML_BYTES_LIMIT: usize = 256 * 1024;
const MAX_REDIRECTS: usize = 3;
const OUTPUT_SIZE: u32 = 32;
const POSITIVE_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const MISSING_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const TRANSIENT_TTL: Duration = Duration::from_secs(5 * 60);
const REQUEST_DEADLINE: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const USER_AGENT: &str = "NoemaFavicon/0.1 (+https://github.com/kpsuperplane/Noema)";

#[derive(Clone)]
pub(super) struct FaviconService {
    inner: Arc<FaviconServiceInner>,
}

struct FaviconServiceInner {
    cache_dir: PathBuf,
    host_locks: Mutex<HashMap<String, Weak<Mutex<()>>>>,
    outbound_slots: Semaphore,
}

#[derive(Debug)]
pub(super) struct Favicon {
    pub(super) png: Vec<u8>,
    pub(super) etag: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FaviconError {
    InvalidHostname,
    Missing,
    Transient,
    Timeout,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CacheOutcome {
    Available,
    Missing,
    Transient,
}

#[derive(Debug, Deserialize, Serialize)]
struct CacheMetadata {
    hostname: String,
    outcome: CacheOutcome,
    fetched_at: u64,
    expires_at: u64,
}

enum CachedValue {
    Fresh(Result<Favicon, FaviconError>),
    Stale(Favicon),
    Miss,
}

impl FaviconService {
    pub(super) fn new(cache_dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(FaviconServiceInner {
                cache_dir,
                host_locks: Mutex::new(HashMap::new()),
                outbound_slots: Semaphore::new(8),
            }),
        }
    }

    pub(super) async fn get(&self, raw_hostname: &str) -> Result<Favicon, FaviconError> {
        let hostname = normalize_hostname(raw_hostname)?;
        match self.read_cache(&hostname) {
            CachedValue::Fresh(result) => result,
            CachedValue::Stale(icon) => {
                let service = self.clone();
                tokio::spawn(async move { service.refresh_if_stale(hostname).await });
                Ok(icon)
            }
            CachedValue::Miss => self.load_or_fetch(hostname).await,
        }
    }

    async fn load_or_fetch(&self, hostname: String) -> Result<Favicon, FaviconError> {
        let host_lock = self.host_lock(&hostname).await;
        let _guard = host_lock.lock().await;
        match self.read_cache(&hostname) {
            CachedValue::Fresh(result) => return result,
            CachedValue::Stale(icon) => return Ok(icon),
            CachedValue::Miss => {}
        }
        self.fetch_and_store(&hostname).await
    }

    async fn refresh_if_stale(&self, hostname: String) {
        let host_lock = self.host_lock(&hostname).await;
        let _guard = host_lock.lock().await;
        if matches!(self.read_cache(&hostname), CachedValue::Fresh(_)) {
            return;
        }
        if let Ok(icon) = self.fetch(&hostname).await {
            self.write_cache(&hostname, CacheOutcome::Available, unix_time(), &icon.png);
        }
    }

    async fn host_lock(&self, hostname: &str) -> Arc<Mutex<()>> {
        let mut host_locks = self.inner.host_locks.lock().await;
        if let Some(lock) = host_locks.get(hostname).and_then(Weak::upgrade) {
            return lock;
        }
        if host_locks.len() >= CACHE_LIMIT {
            host_locks.retain(|_, lock| lock.strong_count() > 0);
        }
        let lock = Arc::new(Mutex::new(()));
        host_locks.insert(hostname.to_string(), Arc::downgrade(&lock));
        lock
    }

    async fn fetch_and_store(&self, hostname: &str) -> Result<Favicon, FaviconError> {
        let result = self.fetch(hostname).await;
        let now = unix_time();
        match &result {
            Ok(icon) => self.write_cache(hostname, CacheOutcome::Available, now, &icon.png),
            Err(FaviconError::Missing) => {
                self.write_cache(hostname, CacheOutcome::Missing, now, &[])
            }
            Err(FaviconError::Transient | FaviconError::Timeout) => {
                self.write_cache(hostname, CacheOutcome::Transient, now, &[])
            }
            Err(FaviconError::InvalidHostname) => {}
        }
        result
    }

    async fn fetch(&self, hostname: &str) -> Result<Favicon, FaviconError> {
        let request = async {
            let _slot = self
                .inner
                .outbound_slots
                .acquire()
                .await
                .map_err(|_| FaviconError::Transient)?;
            let mut last_error = FaviconError::Missing;
            for scheme in ["https", "http"] {
                match fetch_for_scheme(hostname, scheme).await {
                    Ok(png) => return Ok(favicon(png)),
                    Err(error) => last_error = stronger_error(last_error, error),
                }
            }
            Err(last_error)
        };
        tokio::time::timeout(REQUEST_DEADLINE, request)
            .await
            .map_err(|_| FaviconError::Timeout)?
    }

    fn read_cache(&self, hostname: &str) -> CachedValue {
        let key = cache_key(hostname);
        let metadata_path = self.inner.cache_dir.join(format!("{key}.json"));
        let Ok(metadata_bytes) = std::fs::read(metadata_path) else {
            return CachedValue::Miss;
        };
        let Ok(metadata) = serde_json::from_slice::<CacheMetadata>(&metadata_bytes) else {
            return CachedValue::Miss;
        };
        if metadata.hostname != hostname {
            return CachedValue::Miss;
        }
        let fresh = metadata.expires_at > unix_time();
        match metadata.outcome {
            CacheOutcome::Available => {
                let Ok(png) = std::fs::read(self.inner.cache_dir.join(format!("{key}.png"))) else {
                    return CachedValue::Miss;
                };
                let icon = favicon(png);
                if fresh {
                    CachedValue::Fresh(Ok(icon))
                } else {
                    CachedValue::Stale(icon)
                }
            }
            CacheOutcome::Missing if fresh => CachedValue::Fresh(Err(FaviconError::Missing)),
            CacheOutcome::Transient if fresh => CachedValue::Fresh(Err(FaviconError::Transient)),
            CacheOutcome::Missing | CacheOutcome::Transient => CachedValue::Miss,
        }
    }

    fn write_cache(&self, hostname: &str, outcome: CacheOutcome, now: u64, png: &[u8]) {
        if ensure_private_dir(&self.inner.cache_dir).is_err() {
            return;
        }
        let key = cache_key(hostname);
        let png_path = self.inner.cache_dir.join(format!("{key}.png"));
        if outcome == CacheOutcome::Available {
            if atomic_write_private(&png_path, png).is_err() {
                return;
            }
        } else {
            let _ = std::fs::remove_file(&png_path);
        }
        let ttl = match outcome {
            CacheOutcome::Available => POSITIVE_TTL,
            CacheOutcome::Missing => MISSING_TTL,
            CacheOutcome::Transient => TRANSIENT_TTL,
        };
        let metadata = CacheMetadata {
            hostname: hostname.to_string(),
            outcome,
            fetched_at: now,
            expires_at: now.saturating_add(ttl.as_secs()),
        };
        if let Ok(bytes) = serde_json::to_vec(&metadata) {
            let _ = atomic_write_private(&self.inner.cache_dir.join(format!("{key}.json")), &bytes);
        }
        evict_old_entries(&self.inner.cache_dir);
    }
}

fn normalize_hostname(raw_hostname: &str) -> Result<String, FaviconError> {
    let trimmed = raw_hostname.trim();
    let trimmed = trimmed.strip_suffix('.').unwrap_or(trimmed);
    if trimmed.is_empty() || trimmed.len() > 253 {
        return Err(FaviconError::InvalidHostname);
    }
    match Host::parse(trimmed).map_err(|_| FaviconError::InvalidHostname)? {
        Host::Domain(hostname) if !hostname.is_empty() => Ok(hostname.to_ascii_lowercase()),
        Host::Domain(_) | Host::Ipv4(_) | Host::Ipv6(_) => Err(FaviconError::InvalidHostname),
    }
}

async fn fetch_for_scheme(hostname: &str, scheme: &str) -> Result<Vec<u8>, FaviconError> {
    let root = Url::parse(&format!("{scheme}://{hostname}/"))
        .map_err(|_| FaviconError::InvalidHostname)?;
    let direct_url = root
        .join("favicon.ico")
        .map_err(|_| FaviconError::Missing)?;
    let direct = fetch_icon(direct_url);
    let discovered = discover_icon(root);
    tokio::pin!(direct, discovered);
    tokio::select! {
        direct_result = &mut direct => match direct_result {
            Ok(png) => Ok(png),
            Err(direct_error) => match discovered.await {
                Ok(Some(icon_url)) => fetch_icon(icon_url)
                    .await
                    .map_err(|error| stronger_error(direct_error, error)),
                Ok(None) => Err(direct_error),
                Err(error) => Err(stronger_error(direct_error, error)),
            },
        },
        discovery_result = &mut discovered => match discovery_result {
            Ok(Some(icon_url)) => {
                let declared = fetch_icon(icon_url);
                tokio::pin!(declared);
                tokio::select! {
                    direct_result = &mut direct => match direct_result {
                        Ok(png) => Ok(png),
                        Err(direct_error) => declared.await
                            .map_err(|error| stronger_error(direct_error, error)),
                    },
                    declared_result = &mut declared => match declared_result {
                        Ok(png) => Ok(png),
                        Err(declared_error) => direct.await
                            .map_err(|error| stronger_error(declared_error, error)),
                    },
                }
            }
            Ok(None) => direct.await,
            Err(discovery_error) => direct.await
                .map_err(|error| stronger_error(discovery_error, error)),
        },
    }
}

async fn discover_icon(root: Url) -> Result<Option<Url>, FaviconError> {
    let response = fetch_bytes(root, "text/html,application/xhtml+xml", HTML_BYTES_LIMIT).await?;
    if !matches!(
        response.media_type.as_str(),
        "" | "text/html" | "application/xhtml+xml"
    ) {
        return Ok(None);
    }
    let document = Html::parse_document(&String::from_utf8_lossy(&response.bytes));
    let selector = Selector::parse("link[rel][href]").map_err(|_| FaviconError::Missing)?;
    let mut candidates = document
        .select(&selector)
        .enumerate()
        .filter_map(|(index, element)| {
            let rel = element.value().attr("rel")?.to_ascii_lowercase();
            let tokens = rel.split_ascii_whitespace().collect::<Vec<_>>();
            let kind = if tokens.contains(&"icon") && !tokens.contains(&"mask-icon") {
                0
            } else if tokens
                .iter()
                .any(|token| token.starts_with("apple-touch-icon"))
            {
                1
            } else {
                return None;
            };
            let href = element.value().attr("href")?;
            let url = response.final_url.join(href).ok()?;
            Some((
                kind,
                icon_size_distance(element.value().attr("sizes")),
                index,
                url,
            ))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| (candidate.0, candidate.1, candidate.2));
    Ok(candidates.into_iter().next().map(|candidate| candidate.3))
}

fn icon_size_distance(sizes: Option<&str>) -> u32 {
    sizes
        .into_iter()
        .flat_map(str::split_ascii_whitespace)
        .filter_map(|size| size.split_once('x'))
        .filter_map(|(width, height)| {
            Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?))
        })
        .map(|(width, height)| width.abs_diff(OUTPUT_SIZE) + height.abs_diff(OUTPUT_SIZE))
        .min()
        .unwrap_or(u32::MAX)
}

async fn fetch_icon(url: Url) -> Result<Vec<u8>, FaviconError> {
    let response = fetch_bytes(url, "image/*,*/*;q=0.1", ICON_BYTES_LIMIT).await?;
    normalize_image(&response.bytes)
}

struct ByteResponse {
    bytes: Vec<u8>,
    final_url: Url,
    media_type: String,
}

async fn fetch_bytes(url: Url, accept: &str, limit: usize) -> Result<ByteResponse, FaviconError> {
    let mut checked = validate_public_url_parsed(url)
        .await
        .map_err(map_public_error)?;
    for redirect_count in 0..=MAX_REDIRECTS {
        let client =
            checked_public_http_client(&checked, CONNECT_TIMEOUT).map_err(map_public_error)?;
        let response = client
            .get(checked.url.clone())
            .header(header::USER_AGENT, USER_AGENT)
            .header(header::ACCEPT, accept)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    FaviconError::Timeout
                } else {
                    FaviconError::Transient
                }
            })?;
        if response.status().is_redirection() {
            if redirect_count == MAX_REDIRECTS {
                return Err(FaviconError::Transient);
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(FaviconError::Transient)?;
            let next_url = checked
                .url
                .join(location)
                .map_err(|_| FaviconError::Transient)?;
            checked = validate_public_url_parsed(next_url)
                .await
                .map_err(map_public_error)?;
            continue;
        }
        if !response.status().is_success() {
            return Err(if response.status().is_client_error() {
                FaviconError::Missing
            } else {
                FaviconError::Transient
            });
        }
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(FaviconError::Missing);
        }
        let media_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| FaviconError::Transient)?;
            if bytes.len().saturating_add(chunk.len()) > limit {
                return Err(FaviconError::Missing);
            }
            bytes.extend_from_slice(&chunk);
        }
        return Ok(ByteResponse {
            bytes,
            final_url: checked.url,
            media_type,
        });
    }
    Err(FaviconError::Transient)
}

fn normalize_image(bytes: &[u8]) -> Result<Vec<u8>, FaviconError> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader = reader
        .with_guessed_format()
        .map_err(|_| FaviconError::Missing)?;
    let format = reader.format().ok_or(FaviconError::Missing)?;
    if !matches!(
        format,
        ImageFormat::Gif
            | ImageFormat::Ico
            | ImageFormat::Jpeg
            | ImageFormat::Png
            | ImageFormat::WebP
    ) {
        return Err(FaviconError::Missing);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(1_024);
    limits.max_image_height = Some(1_024);
    limits.max_alloc = Some(4 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| FaviconError::Missing)?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 || width > 1_024 || height > 1_024 {
        return Err(FaviconError::Missing);
    }
    let resized = image.thumbnail(OUTPUT_SIZE, OUTPUT_SIZE).to_rgba8();
    let mut output = RgbaImage::new(OUTPUT_SIZE, OUTPUT_SIZE);
    let x = i64::from((OUTPUT_SIZE - resized.width()) / 2);
    let y = i64::from((OUTPUT_SIZE - resized.height()) / 2);
    imageops::overlay(&mut output, &resized, x, y);
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(output)
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|_| FaviconError::Missing)?;
    Ok(encoded.into_inner())
}

fn map_public_error(error: WebFetchError) -> FaviconError {
    match error {
        WebFetchError::Timeout => FaviconError::Timeout,
        WebFetchError::BlockedTarget
        | WebFetchError::RedirectBlocked
        | WebFetchError::UnsupportedScheme
        | WebFetchError::MalformedUrl => FaviconError::Missing,
        _ => FaviconError::Transient,
    }
}

fn stronger_error(left: FaviconError, right: FaviconError) -> FaviconError {
    if matches!(left, FaviconError::Timeout) || matches!(right, FaviconError::Timeout) {
        FaviconError::Timeout
    } else if matches!(left, FaviconError::Transient) || matches!(right, FaviconError::Transient) {
        FaviconError::Transient
    } else {
        FaviconError::Missing
    }
}

fn favicon(png: Vec<u8>) -> Favicon {
    Favicon {
        etag: format!("\"{}\"", sha256_hex(&png)),
        png,
    }
}

fn cache_key(hostname: &str) -> String {
    sha256_hex(hostname.as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    digest::digest(&digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn evict_old_entries(cache_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return;
    };
    let mut metadata = entries
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "json")
        })
        .filter_map(|entry| {
            let bytes = std::fs::read(entry.path()).ok()?;
            let value = serde_json::from_slice::<CacheMetadata>(&bytes).ok()?;
            Some((value.fetched_at, entry.path()))
        })
        .collect::<Vec<_>>();
    if metadata.len() <= CACHE_LIMIT {
        return;
    }
    metadata.sort_by_key(|entry| entry.0);
    let remove_count = metadata.len() - CACHE_LIMIT;
    for (_, path) in metadata.into_iter().take(remove_count) {
        let png_path = path.with_extension("png");
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(png_path);
    }
}

#[cfg(test)]
pub(in crate::web) use tests::seed_icon;

#[cfg(test)]
mod tests {
    use super::*;

    pub(in crate::web) fn seed_icon(service: &FaviconService, hostname: &str, png: &[u8]) {
        service.write_cache(hostname, CacheOutcome::Available, unix_time(), png);
    }

    #[test]
    fn hostname_normalization_keeps_exact_hosts_distinct() {
        assert_eq!(normalize_hostname("EXAMPLE.com.").unwrap(), "example.com");
        assert_eq!(
            normalize_hostname("www.example.com").unwrap(),
            "www.example.com"
        );
        assert_ne!(cache_key("www.example.com"), cache_key("example.com"));
    }

    #[test]
    fn hostname_normalization_rejects_ip_and_authority_values() {
        for hostname in [
            "127.0.0.1",
            "[::1]",
            "example.com:443",
            "user@example.com",
            "",
        ] {
            assert_eq!(
                normalize_hostname(hostname),
                Err(FaviconError::InvalidHostname)
            );
        }
    }

    #[test]
    fn image_normalization_bounds_and_converts_raster_input() {
        let input = DynamicImage::ImageRgba8(RgbaImage::new(48, 24));
        let mut bytes = Cursor::new(Vec::new());
        input.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let normalized = normalize_image(bytes.get_ref()).expect("normalized PNG");
        let output = image::load_from_memory_with_format(&normalized, ImageFormat::Png).unwrap();
        assert_eq!(output.dimensions(), (OUTPUT_SIZE, OUTPUT_SIZE));
        assert_eq!(normalize_image(b"<svg/>"), Err(FaviconError::Missing));
        let oversized = DynamicImage::ImageRgba8(RgbaImage::new(1_025, 1));
        let mut bytes = Cursor::new(Vec::new());
        oversized.write_to(&mut bytes, ImageFormat::Png).unwrap();
        assert_eq!(normalize_image(bytes.get_ref()), Err(FaviconError::Missing));
    }

    #[test]
    fn cache_preserves_positive_and_negative_outcomes() {
        let directory = tempfile::tempdir().unwrap();
        let service = FaviconService::new(directory.path().to_path_buf());
        let source = {
            let mut bytes = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(RgbaImage::new(1, 1))
                .write_to(&mut bytes, ImageFormat::Png)
                .unwrap();
            bytes.into_inner()
        };
        let png = normalize_image(&source).unwrap();
        service.write_cache("example.com", CacheOutcome::Available, unix_time(), &png);
        assert!(matches!(
            service.read_cache("example.com"),
            CachedValue::Fresh(Ok(_))
        ));
        service.write_cache("missing.example", CacheOutcome::Missing, unix_time(), &[]);
        assert!(matches!(
            service.read_cache("missing.example"),
            CachedValue::Fresh(Err(FaviconError::Missing))
        ));
        service.write_cache(
            "transient.example",
            CacheOutcome::Transient,
            unix_time(),
            &[],
        );
        assert!(matches!(
            service.read_cache("transient.example"),
            CachedValue::Fresh(Err(FaviconError::Transient))
        ));
        service.write_cache("stale.example", CacheOutcome::Available, 1, &png);
        assert!(matches!(
            service.read_cache("stale.example"),
            CachedValue::Stale(_)
        ));
    }

    #[tokio::test]
    async fn concurrent_requests_share_one_hostname_fetch_lock() {
        let directory = tempfile::tempdir().unwrap();
        let service = FaviconService::new(directory.path().to_path_buf());
        let (first, second) = tokio::join!(
            service.host_lock("example.com"),
            service.host_lock("example.com")
        );
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn cache_write_evicts_the_oldest_hostname() {
        let directory = tempfile::tempdir().unwrap();
        let service = FaviconService::new(directory.path().to_path_buf());
        for index in 0..=CACHE_LIMIT {
            service.write_cache(
                &format!("{index}.example"),
                CacheOutcome::Missing,
                index as u64 + 1,
                &[],
            );
        }
        assert!(
            !directory
                .path()
                .join(format!("{}.json", cache_key("0.example")))
                .exists()
        );
        assert!(
            directory
                .path()
                .join(format!("{}.json", cache_key("1024.example")))
                .exists()
        );
    }
}
