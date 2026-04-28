use anyhow::{anyhow, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use rand::RngExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

#[derive(Debug)]
pub struct SegmentResult {
    pub url: String,
    pub path: PathBuf,
    pub id: usize,
    pub success: bool,
    pub error: Option<String>,
}

/// Download all segments in parallel and merge them into a single file.
///
/// Segments are downloaded into a private temporary directory under the system
/// temp folder (e.g. `/tmp/tdl-seg-<random>/`).  The directory is isolated per
/// call so concurrent track downloads never share segment file names.  After a
/// successful merge the temp directory is removed entirely; on failure it is
/// also cleaned up to avoid leaving debris in the OS temp folder.
pub async fn download_and_merge(
    urls: &[String],
    output_path: &Path,
    http_client: &reqwest::Client,
    max_concurrent: usize,
    progress: Option<&indicatif::ProgressBar>,
) -> Result<()> {
    if urls.is_empty() {
        return Err(anyhow!("No segment URLs provided"));
    }

    // Use a unique temp directory per download so parallel tracks can't collide.
    let temp_dir = std::env::temp_dir().join(format!("tdl-seg-{}", unique_id()));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| anyhow!("Failed to create temp dir {}: {e}", temp_dir.display()))?;
    let temp_dir = temp_dir.as_path();

    // Create a semaphore-like pool using FuturesUnordered with bounded concurrency.
    let mut futures: FuturesUnordered<_> = FuturesUnordered::new();
    let mut urls_iter = urls.iter().enumerate().peekable();

    // Seed the pool with up to max_concurrent tasks.
    for _ in 0..max_concurrent {
        if let Some((seg_id, url)) = urls_iter.next() {
            futures.push(download_segment(
                url.clone(),
                seg_id,
                temp_dir,
                http_client,
                5,
            ));
        }
    }

    let mut results: Vec<SegmentResult> = Vec::with_capacity(urls.len());

    // As each task completes, submit a new one to keep concurrency bounded.
    while let Some(result) = futures.next().await {
        let result = result?;

        if let Some(pb) = progress {
            pb.inc(1);
        }

        if !result.success {
            // Clean up the temp directory before returning the error.
            let _ = tokio::fs::remove_dir_all(temp_dir).await;
            return Err(anyhow!(
                "Segment {} failed to download: {}",
                result.id,
                result.error.as_deref().unwrap_or("unknown error")
            ));
        }

        // Submit the next URL if there are more.
        if let Some((seg_id, url)) = urls_iter.next() {
            futures.push(download_segment(
                url.clone(),
                seg_id,
                temp_dir,
                http_client,
                5,
            ));
        }

        results.push(result);
    }

    // Sort by segment ID to ensure correct order.
    results.sort_by_key(|r| r.id);

    // Merge sorted segments into the output file, then remove the entire temp dir.
    let merge_result = merge_segments(&results, output_path).await;
    let _ = tokio::fs::remove_dir_all(temp_dir).await;
    merge_result
}

/// Download a single segment with retry and exponential backoff.
///
/// On each retry failure the delay doubles (1s, 2s, 4s, 8s, 16s).
/// The segment body is written to a temporary file in `temp_dir`.
/// The segment ID is parsed from the URL filename when possible; otherwise
/// the provided `seg_id` is used.
async fn download_segment(
    url: String,
    seg_id: usize,
    temp_dir: &Path,
    http_client: &reqwest::Client,
    max_retries: u32,
) -> Result<SegmentResult> {
    let parsed_id = parse_segment_id(&url);
    let effective_id = if parsed_id > 0 { parsed_id } else { seg_id };

    let temp_path = temp_dir.join(format!("segment_{effective_id}.tmp"));

    let mut last_error: Option<String> = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = tokio::time::Duration::from_secs(1 << (attempt - 1));
            tokio::time::sleep(delay).await;
        }

        match http_client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    last_error = Some(format!("HTTP {status}"));
                    continue;
                }

                let bytes = match resp.bytes().await {
                    Ok(b) => b,
                    Err(e) => {
                        last_error = Some(format!("Failed to read response body: {e}"));
                        continue;
                    }
                };

                let mut file = match tokio::fs::File::create(&temp_path).await {
                    Ok(f) => f,
                    Err(e) => {
                        last_error = Some(format!("Failed to create temp file: {e}"));
                        continue;
                    }
                };

                if let Err(e) = file.write_all(&bytes).await {
                    last_error = Some(format!("Failed to write segment data: {e}"));
                    continue;
                }

                if let Err(e) = file.flush().await {
                    last_error = Some(format!("Failed to flush segment data: {e}"));
                    continue;
                }

                return Ok(SegmentResult {
                    url,
                    path: temp_path,
                    id: effective_id,
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                last_error = Some(format!("Request failed: {e}"));
                continue;
            }
        }
    }

    Ok(SegmentResult {
        url,
        path: temp_path,
        id: effective_id,
        success: false,
        error: last_error,
    })
}

/// Merge sorted segment files into a single output file.
///
/// Segment cleanup is handled by the caller, which removes the entire temp
/// directory after this function returns.
async fn merge_segments(segments: &[SegmentResult], output_path: &Path) -> Result<()> {
    let mut output_file = tokio::fs::File::create(output_path)
        .await
        .map_err(|e| anyhow!("Failed to create output file {}: {e}", output_path.display()))?;

    for seg in segments {
        let data = tokio::fs::read(&seg.path)
            .await
            .map_err(|e| anyhow!("Failed to read segment file {}: {e}", seg.path.display()))?;

        output_file
            .write_all(&data)
            .await
            .map_err(|e| anyhow!("Failed to write segment data to output: {e}"))?;
    }

    output_file
        .flush()
        .await
        .map_err(|e| anyhow!("Failed to flush output file: {e}"))?;

    Ok(())
}

/// Parse segment ID from a URL's filename.
///
/// Extracts the filename portion of the URL (the last path component before any
/// Generate a short random hex string for temp directory names.
fn unique_id() -> String {
    let n: u64 = rand::rng().random();
    format!("{n:016x}")
}

/// Parse segment ID from a URL's filename.
///
/// Extracts the filename portion of the URL (the last path component before any
/// query string), then attempts to parse a trailing numeric ID separated by '_'.
/// For example, `https://cdn.example.com/seg_42.ts?token=abc` yields `42`.
/// Returns 0 when no numeric suffix can be parsed.
fn parse_segment_id(url: &str) -> usize {
    // Strip query string and fragment.
    let path_part = url.split('?').next().unwrap_or(url);
    let path_part = path_part.split('#').next().unwrap_or(path_part);

    // Take the last path component.
    let filename = path_part
        .rsplit('/')
        .next()
        .unwrap_or(path_part);

    // Strip common file extensions so that "seg_42.ts" becomes "seg_42".
    let stem = filename
        .rsplit_once('.')
        .map(|(name, _ext)| name)
        .unwrap_or(filename);

    // Split by '_' and try to parse the last piece as an integer.
    // Only consider the underscore-split result when the filename actually
    // contains at least one underscore; otherwise the trailing number is
    // part of the filename itself, not a segment ID.
    if stem.contains('_') {
        stem.rsplit('_')
            .next()
            .and_then(|part| part.parse::<usize>().ok())
            .unwrap_or(0)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_segment_id_with_extension() {
        assert_eq!(
            parse_segment_id("https://cdn.example.com/segment_42.ts?token=abc"),
            42
        );
    }

    #[test]
    fn parse_segment_id_no_extension() {
        assert_eq!(
            parse_segment_id("https://cdn.example.com/seg_7"),
            7
        );
    }

    #[test]
    fn parse_segment_id_no_underscore() {
        assert_eq!(parse_segment_id("https://cdn.example.com/42.ts"), 0);
    }

    #[test]
    fn parse_segment_id_trailing_text() {
        assert_eq!(
            parse_segment_id("https://cdn.example.com/segment_abc.ts"),
            0
        );
    }

    #[test]
    fn parse_segment_id_multiple_underscores() {
        assert_eq!(
            parse_segment_id("https://cdn.example.com/prefix_name_99.ts"),
            99
        );
    }

    #[test]
    fn parse_segment_id_fragment() {
        assert_eq!(
            parse_segment_id("https://cdn.example.com/seg_3.ts#fragment"),
            3
        );
    }

    #[test]
    fn parse_segment_id_zero() {
        assert_eq!(
            parse_segment_id("https://cdn.example.com/seg_0.ts"),
            0
        );
    }
}
