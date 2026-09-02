//! Fast, bounded yt-dlp metadata search supervision.

use std::{
    collections::{HashMap, HashSet},
    io::{self, Read},
    process::{Child, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use tracing::info;

use crate::{
    error::AppError,
    importer::{self, CandidateKind, ImportCandidate},
};

const MAX_CONCURRENT_SEARCHES: usize = 2;
const MAX_QUERY_BYTES: usize = 180;
const MAX_STDOUT_BYTES: usize = 512 * 1024;
const MAX_STDERR_BYTES: usize = 32 * 1024;
const SEARCH_TIMEOUT: Duration = Duration::from_secs(12);
const SEARCH_RESULT_COUNT: usize = 10;
const PUBLISHED_RESULT_COUNT: usize = 5;

#[derive(Clone, Default)]
pub struct YoutubeSearchService {
    inner: Arc<SearchInner>,
}

#[derive(Default)]
struct SearchInner {
    generation: AtomicU64,
    next_child_id: AtomicU64,
    children: Mutex<HashMap<u64, Arc<Mutex<Child>>>>,
    active_count: Mutex<usize>,
    slot_available: Condvar,
}

struct SearchPermit {
    inner: Arc<SearchInner>,
}

impl Drop for SearchPermit {
    fn drop(&mut self) {
        if let Ok(mut active) = self.inner.active_count.lock() {
            *active = active.saturating_sub(1);
            self.inner.slot_available.notify_one();
        }
    }
}

struct BoundedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

impl YoutubeSearchService {
    pub fn begin(&self) -> u64 {
        let generation = self.inner.generation.fetch_add(1, Ordering::AcqRel) + 1;
        if let Ok(mut children) = self.inner.children.lock() {
            for child in children.drain().map(|(_, child)| child) {
                if let Ok(mut child) = child.lock() {
                    let _ = child.kill();
                }
            }
        }
        self.inner.slot_available.notify_all();
        generation
    }

    pub fn resolve(&self, query: &str, generation: u64) -> Result<Vec<ImportCandidate>, AppError> {
        let query = query.trim();
        if query.is_empty() || query.len() > MAX_QUERY_BYTES {
            return Err(AppError::BackgroundTask(
                "YouTube search must contain between 1 and 180 bytes".into(),
            ));
        }
        ensure_current(&self.inner, generation)?;
        let _permit = acquire_slot(Arc::clone(&self.inner), generation)?;
        ensure_current(&self.inner, generation)?;

        let started = Instant::now();
        let tool = importer::ytdlp_command()?;
        let mut child = tool
            .command()
            .args([
                "--ignore-config",
                "--flat-playlist",
                "--dump-json",
                "--playlist-end",
                "10",
                "--",
            ])
            .arg(format!("ytsearch{SEARCH_RESULT_COUNT}:{query}"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                AppError::BackgroundTask(format!("could not start fast yt-dlp search: {error}"))
            })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::BackgroundTask("yt-dlp search output is unavailable".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AppError::BackgroundTask("yt-dlp search diagnostics are unavailable".into())
        })?;
        let child_id = self.inner.next_child_id.fetch_add(1, Ordering::Relaxed);
        let child = Arc::new(Mutex::new(child));
        self.inner
            .children
            .lock()
            .map_err(|_| AppError::BackgroundTask("YouTube search state is unavailable".into()))?
            .insert(child_id, Arc::clone(&child));
        if ensure_current(&self.inner, generation).is_err() {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
            }
        }

        let stdout_reader = thread::spawn(move || read_bounded(stdout, MAX_STDOUT_BYTES));
        let stderr_reader = thread::spawn(move || read_bounded(stderr, MAX_STDERR_BYTES));
        let status = wait_for_child(&child, SEARCH_TIMEOUT);
        clear_child(&self.inner, child_id);
        let stdout = stdout_reader
            .join()
            .map_err(|_| AppError::BackgroundTask("yt-dlp search output reader failed".into()))?
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| AppError::BackgroundTask("yt-dlp search diagnostic reader failed".into()))?
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        ensure_current(&self.inner, generation)?;
        let status = status?;
        if stdout.exceeded {
            return Err(AppError::BackgroundTask(
                "yt-dlp search output exceeded 512 KiB".into(),
            ));
        }
        if !status.success() {
            let diagnostic = String::from_utf8_lossy(&stderr.bytes).trim().to_owned();
            return Err(AppError::BackgroundTask(if diagnostic.is_empty() {
                "yt-dlp search failed without diagnostics".into()
            } else {
                diagnostic
            }));
        }
        let candidates = parse_candidates(&stdout.bytes, query);
        info!(
            elapsed_ms = started.elapsed().as_millis(),
            result_count = candidates.len(),
            "YouTube search completed"
        );
        Ok(candidates)
    }
}

fn acquire_slot(inner: Arc<SearchInner>, generation: u64) -> Result<SearchPermit, AppError> {
    let mut active = inner
        .active_count
        .lock()
        .map_err(|_| AppError::BackgroundTask("YouTube search slots are unavailable".into()))?;
    while *active >= MAX_CONCURRENT_SEARCHES {
        ensure_current(&inner, generation)?;
        active = inner
            .slot_available
            .wait(active)
            .map_err(|_| AppError::BackgroundTask("YouTube search slots are unavailable".into()))?;
    }
    ensure_current(&inner, generation)?;
    *active += 1;
    drop(active);
    Ok(SearchPermit { inner })
}

fn ensure_current(inner: &SearchInner, generation: u64) -> Result<(), AppError> {
    (inner.generation.load(Ordering::Acquire) == generation)
        .then_some(())
        .ok_or_else(|| AppError::BackgroundTask("YouTube search was cancelled or replaced".into()))
}

fn wait_for_child(
    child: &Arc<Mutex<Child>>,
    timeout: Duration,
) -> Result<std::process::ExitStatus, AppError> {
    let started = Instant::now();
    loop {
        let status = child
            .lock()
            .map_err(|_| AppError::BackgroundTask("yt-dlp search process is unavailable".into()))?
            .try_wait()
            .map_err(|error| AppError::BackgroundTask(error.to_string()))?;
        if let Some(status) = status {
            return Ok(status);
        }
        if started.elapsed() >= timeout {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
            return Err(AppError::BackgroundTask(
                "YouTube search timed out after 12 seconds".into(),
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn clear_child(inner: &SearchInner, child_id: u64) {
    if let Ok(mut children) = inner.children.lock() {
        children.remove(&child_id);
    }
}

fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<BoundedOutput> {
    let mut bytes = Vec::with_capacity(limit.min(32 * 1024));
    let mut exceeded = false;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        bytes.extend_from_slice(&buffer[..count.min(remaining)]);
        exceeded |= count > remaining;
    }
    Ok(BoundedOutput { bytes, exceeded })
}

fn parse_candidates(stdout: &[u8], query: &str) -> Vec<ImportCandidate> {
    let mut candidates = String::from_utf8_lossy(stdout)
        .lines()
        .enumerate()
        .filter_map(|(search_index, line)| {
            let value: serde_json::Value = serde_json::from_str(line).ok()?;
            let id = value.get("id")?.as_str()?;
            if id.is_empty()
                || id.len() > 64
                || !id.bytes().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, b'-' | b'_')
                })
            {
                return None;
            }
            let title = clean_youtube_title(
                &bounded_text(
                    value
                        .get("title")
                        .and_then(|value| value.as_str())
                        .unwrap_or(query),
                    256,
                ),
                id,
            );
            let channel = bounded_text(
                value
                    .get("channel")
                    .or_else(|| value.get("uploader"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("YouTube"),
                160,
            );
            let views = value.get("view_count").and_then(|value| value.as_u64());
            let verified = value
                .get("channel_is_verified")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let score = relevance_score(query, &title, &channel, views, verified, search_index);
            Some((
                score,
                ImportCandidate {
                    input: format!("https://www.youtube.com/watch?v={id}"),
                    title,
                    detail: channel,
                    kind: CandidateKind::Video,
                    match_score: Some(score),
                    thumbnail_url: Some(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")),
                    video_id: Some(id.to_owned()),
                },
            ))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen_titles = HashSet::new();
    candidates
        .into_iter()
        .filter(|(_, candidate)| {
            let normalized_title = tokens(&candidate.title).join("\u{1f}");
            normalized_title.is_empty() || seen_titles.insert(normalized_title)
        })
        .take(PUBLISHED_RESULT_COUNT)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn clean_youtube_title(title: &str, video_id: &str) -> String {
    let suffix = format!(" [{video_id}]");
    title
        .strip_suffix(&suffix)
        .unwrap_or(title)
        .trim()
        .to_owned()
}

fn bounded_text(value: &str, maximum_characters: usize) -> String {
    value.chars().take(maximum_characters).collect()
}

fn relevance_score(
    query: &str,
    title: &str,
    channel: &str,
    views: Option<u64>,
    verified: bool,
    search_index: usize,
) -> f64 {
    let query_tokens = tokens(query);
    let title_tokens = tokens(title);
    let channel_tokens = tokens(channel);
    let mut combined_tokens = title_tokens.clone();
    combined_tokens.extend(channel_tokens.iter().cloned());

    let combined_match = token_recall(&query_tokens, &combined_tokens);
    let title_match = token_recall(&query_tokens, &title_tokens);
    let title_precision = token_recall(&title_tokens, &query_tokens);
    let channel_match = token_recall(&query_tokens, &channel_tokens);
    let structured = split_artist_and_title(query);
    let normalized_channel = channel.to_lowercase();
    let official_name_hint = ["official", "vevo", "topic"]
        .iter()
        .any(|marker| normalized_channel.contains(marker));
    let popularity = views
        .map(|count| ((count as f64 + 1.0).log10() / 9.0).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let position_hint = (1.0 - search_index as f64 / SEARCH_RESULT_COUNT as f64).max(0.0);

    // yt-dlp maps both YouTube's verification check and Official Artist Channel
    // music-note badge to `channel_is_verified` in flat search results.
    let metadata_score = 0.03 * f64::from(verified)
        + 0.005 * f64::from(official_name_hint)
        + 0.01 * popularity
        + 0.005 * position_hint;
    let mut score = if let Some((artist, expected_title)) = structured {
        let artist_tokens = tokens(artist);
        let artist_match = token_recall(&artist_tokens, &title_tokens)
            .max(token_recall(&artist_tokens, &channel_tokens));
        0.42 * token_recall(&tokens(expected_title), &title_tokens)
            + 0.24 * artist_match
            + 0.17 * title_match
            + 0.07 * combined_match
            + 0.05 * title_precision
            + metadata_score
    } else {
        0.58 * title_match
            + 0.24 * title_precision
            + 0.10 * combined_match
            + 0.03 * channel_match
            + metadata_score
    };

    const VERSION_MARKERS: &[&str] = &[
        "cover",
        "karaoke",
        "tutorial",
        "reaction",
        "remix",
        "nightcore",
        "sped",
        "slowed",
        "live",
        "instrumental",
    ];
    let mismatch_count = VERSION_MARKERS
        .iter()
        .filter(|marker| {
            !query_tokens.iter().any(|token| token == **marker)
                && title_tokens.iter().any(|token| token == **marker)
        })
        .count();
    score -= (mismatch_count as f64 * 0.12).min(0.36);
    score.clamp(0.01, 0.99)
}

fn split_artist_and_title(query: &str) -> Option<(&str, &str)> {
    [" - ", " – ", " — "]
        .iter()
        .find_map(|separator| query.split_once(separator))
        .filter(|(artist, title)| !artist.trim().is_empty() && !title.trim().is_empty())
}

fn tokens(value: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut current_characters = 0;
    for character in value.to_lowercase().chars() {
        if character.is_alphanumeric() {
            if current_characters < 64 {
                current.push(character);
                current_characters += 1;
            }
        } else if !matches!(character, '\'' | '’' | '‘' | 'ʼ') {
            push_token(&mut result, &mut current);
            current_characters = 0;
            if result.len() == 32 {
                return result;
            }
        }
    }
    push_token(&mut result, &mut current);
    result
}

fn push_token(result: &mut Vec<String>, current: &mut String) {
    if !current.is_empty()
        && !matches!(
            current.as_str(),
            "official" | "audio" | "video" | "hd" | "4k"
        )
        && result.len() < 32
    {
        result.push(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn token_recall(expected: &[String], actual: &[String]) -> f64 {
    if expected.is_empty() {
        return 0.0;
    }
    expected
        .iter()
        .map(|expected_token| {
            actual
                .iter()
                .map(|actual_token| token_similarity(expected_token, actual_token))
                .fold(0.0, f64::max)
        })
        .sum::<f64>()
        / expected.len() as f64
}

fn token_similarity(left: &str, right: &str) -> f64 {
    if left == right {
        return 1.0;
    }
    let longest = left.chars().count().max(right.chars().count());
    if longest < 4 {
        return 0.0;
    }
    let distance = levenshtein(left, right);
    let similarity = 1.0 - distance as f64 / longest as f64;
    if similarity >= 0.72 {
        similarity
    } else {
        0.0
    }
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    for (left_index, left_character) in left.chars().enumerate() {
        let mut current = vec![left_index + 1];
        for (right_index, right_character) in right.iter().enumerate() {
            current.push(
                (current[right_index] + 1)
                    .min(previous[right_index + 1] + 1)
                    .min(previous[right_index] + usize::from(left_character != *right_character)),
            );
        }
        previous = current;
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_bounded_valid_video_results() {
        let output = br#"{"id":"abc","title":"Song","channel":"Artist"}
{"title":"Missing id"}
not json
"#;
        let candidates = parse_candidates(output, "fallback");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].input, "https://www.youtube.com/watch?v=abc");
        assert_eq!(candidates[0].title, "Song");
        assert_eq!(candidates[0].detail, "Artist");
        assert_eq!(
            candidates[0].thumbnail_url.as_deref(),
            Some("https://i.ytimg.com/vi/abc/mqdefault.jpg")
        );
        assert_eq!(candidates[0].video_id.as_deref(), Some("abc"));
        assert!(candidates[0].match_score.is_some());
    }

    #[test]
    fn removes_only_a_trailing_bracketed_copy_of_the_video_id() {
        assert_eq!(
            clean_youtube_title("Bold as Love [9qIunneAx6Y]", "9qIunneAx6Y"),
            "Bold as Love"
        );
        assert_eq!(
            clean_youtube_title("Bold as Love [Live]", "9qIunneAx6Y"),
            "Bold as Love [Live]"
        );
        assert_eq!(
            clean_youtube_title("[9qIunneAx6Y] Bold as Love", "9qIunneAx6Y"),
            "[9qIunneAx6Y] Bold as Love"
        );
    }

    #[test]
    fn ranks_an_artist_channel_above_an_unrequested_cover() {
        let output = br#"{"id":"cover","title":"Enjoy the Silence cover","channel":"Random Guitar","view_count":9000000}
{"id":"official","title":"Enjoy the Silence","channel":"Depeche Mode","channel_is_verified":true,"view_count":1000}
"#;
        let candidates = parse_candidates(output, "Depeche Mode - Enjoy the Silence");

        assert_eq!(
            candidates[0].input,
            "https://www.youtube.com/watch?v=official"
        );
        assert!(candidates[0].match_score > candidates[1].match_score);
    }

    #[test]
    fn exact_title_matches_ignore_case_and_punctuation() {
        let score = relevance_score("The Pot", "tHE, pOT!", "Unrelated channel", None, false, 9);

        assert!(score > 0.9);
    }

    #[test]
    fn apostrophes_have_no_effect_on_an_exact_match() {
        let score = relevance_score(
            "Guns N' Roses - Don't Cry",
            "GUNS N ROSES — DONT CRY",
            "Guns N' Roses",
            None,
            false,
            9,
        );

        assert!(score > 0.9);
    }

    #[test]
    fn artist_and_title_order_does_not_distort_relevance() {
        let canonical = relevance_score(
            "Tool - The Pot",
            "TOOL - The Pot (Official Audio)",
            "Tool",
            Some(1_000_000),
            true,
            0,
        );
        let reversed = relevance_score(
            "Tool - The Pot",
            "The Pot - TOOL",
            "Tool",
            Some(1_000_000),
            true,
            0,
        );

        assert!(canonical > 0.95);
        assert!(reversed > 0.95);
        assert!((canonical - reversed).abs() < f64::EPSILON);
    }

    #[test]
    fn a_verified_artist_badge_outweighs_an_official_word_in_the_channel_name() {
        let verified = relevance_score(
            "Tool - The Pot",
            "Tool - The Pot",
            "Tool",
            Some(1_000_000),
            true,
            0,
        );
        let claimed = relevance_score(
            "Tool - The Pot",
            "Tool - The Pot",
            "Tool Official Uploads",
            Some(1_000_000),
            false,
            0,
        );

        assert!(verified > claimed);
    }

    #[test]
    fn collapses_titles_that_differ_only_by_case_punctuation_or_presentation_markers() {
        let output =
            br#"{"id":"mirror","title":"tool  the pot (Official Audio)","channel":"Mirror"}
{"id":"official","title":"Tool - The Pot","channel":"Tool","channel_is_verified":true}
{"id":"reversed","title":"The Pot - TOOL","channel":"Tool"}
"#;
        let candidates = parse_candidates(output, "Tool - The Pot");

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0].input,
            "https://www.youtube.com/watch?v=official"
        );
        assert_eq!(
            candidates[1].input,
            "https://www.youtube.com/watch?v=reversed"
        );
    }

    #[test]
    fn publishes_only_the_five_highest_scoring_results() {
        let output = (0..10)
            .map(|index| format!(r#"{{"id":"{index}","title":"Song {index}","channel":"Artist"}}"#))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(parse_candidates(output.as_bytes(), "Artist Song").len(), 5);
    }

    #[test]
    fn a_new_generation_invalidates_previous_searches() {
        let service = YoutubeSearchService::default();
        let previous = service.begin();
        let current = service.begin();
        assert!(ensure_current(&service.inner, previous).is_err());
        assert!(ensure_current(&service.inner, current).is_ok());
    }
}
