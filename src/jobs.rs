use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::str::FromStr;

use crate::transcribe::{
    ExportFormat, MediaMetadata, MediaSource, Transcript, TranscriptFeature, TranscriptionModel,
};

/// Lifecycle of a transcription job.
///
/// `Queued` and `Downloading` are separate because the two fail for entirely
/// different reasons — a full queue is ours to fix, a dead or private video is
/// the caller's — and collapsing them produces a "something went wrong" that
/// helps nobody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Downloading,
    Transcribing,
    /// Diarisation, cleanup, summarisation, translation.
    Enriching,
    /// Rendering the requested export formats.
    Rendering,
    Complete,
    Failed,
    /// Cancelled by the caller. Credits for the unprocessed remainder are
    /// refunded; what was already transcribed is kept and stays billed.
    Cancelled,
}

impl JobStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete | Self::Failed | Self::Cancelled)
    }

    pub fn is_running(&self) -> bool {
        !self.is_terminal()
    }

    /// Rough completion fraction for a progress bar, used until the worker
    /// reports a real figure. Ordering matches the enum's declaration order.
    pub fn nominal_progress(&self) -> f64 {
        match self {
            Self::Queued => 0.02,
            Self::Downloading => 0.12,
            Self::Transcribing => 0.45,
            Self::Enriching => 0.80,
            Self::Rendering => 0.94,
            Self::Complete => 1.0,
            Self::Failed | Self::Cancelled => 1.0,
        }
    }

    pub fn to_name(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Downloading => "Fetching media",
            Self::Transcribing => "Transcribing",
            Self::Enriching => "Enriching",
            Self::Rendering => "Rendering",
            Self::Complete => "Complete",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Transcribing => "transcribing",
            Self::Enriching => "enriching",
            Self::Rendering => "rendering",
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for JobStatus {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "queued" => Ok(Self::Queued),
            "downloading" => Ok(Self::Downloading),
            "transcribing" => Ok(Self::Transcribing),
            "enriching" => Ok(Self::Enriching),
            "rendering" => Ok(Self::Rendering),
            "complete" | "completed" => Ok(Self::Complete),
            "failed" => Ok(Self::Failed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            _ => Err(()),
        }
    }
}

impl From<String> for JobStatus {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(JobStatus::Queued)
    }
}

/// Why a job failed, in a form a caller can branch on.
///
/// A string reason is unactionable: a client cannot tell "retry later" from
/// "this video will never work" without parsing prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobFailureKind {
    /// Video is private, deleted, members-only or region-locked.
    MediaUnavailable,
    /// Video exists but is longer than [`crate::transcribe::MAX_MEDIA_DURATION_SECS`].
    MediaTooLong,
    /// URL did not resolve to media we can read.
    UnsupportedSource,
    /// No speech detected in the audio at all.
    NoSpeech,
    /// Account ran out of credits mid-job.
    InsufficientCredits,
    /// Transcription service errored. Retryable.
    TranscriptionFailed,
    /// Download or fetch timed out. Retryable.
    Timeout,
    Internal,
}

impl JobFailureKind {
    /// Whether retrying the identical job could succeed.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::TranscriptionFailed | Self::Timeout | Self::Internal)
    }

    /// Whether the caller should be refunded. Our failures are refunded; a
    /// private video is not our fault but nothing was transcribed either, so
    /// it is refunded too. Running out of credits mid-job is the one case
    /// where the work done so far stays billed.
    pub fn refunds_credits(&self) -> bool {
        !matches!(self, Self::InsufficientCredits)
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::MediaUnavailable => {
                "This video is private, deleted, or not available in our region. \
                 Public and unlisted videos work."
            }
            Self::MediaTooLong => "This video is longer than the 12 hour limit.",
            Self::UnsupportedSource => "That link does not point at media we can read.",
            Self::NoSpeech => "No speech was detected in this audio.",
            Self::InsufficientCredits => "You ran out of minutes part-way through this job.",
            Self::TranscriptionFailed => "Transcription failed. This one is safe to retry.",
            Self::Timeout => "Fetching the media timed out. This one is safe to retry.",
            Self::Internal => "Something went wrong on our side.",
        }
    }

    /// The message once retrying has been given up on.
    ///
    /// [`message`](Self::message) invites a retry for the transient kinds,
    /// which is right until the attempt cap is reached and `retryable` goes
    /// false. Telling someone a failure is "safe to retry" next to a disabled
    /// retry button is worse than saying nothing: they conclude the button is
    /// broken rather than that we gave up. For kinds that were never
    /// retryable this is the same text as `message`, because nothing was
    /// promised.
    pub fn exhausted_message(&self) -> &'static str {
        match self {
            Self::TranscriptionFailed => {
                "Transcription failed on every attempt. Something about this \
                 recording is defeating our pipeline; send us the job id and \
                 we will look at it."
            }
            Self::Timeout => {
                "Fetching the media timed out on every attempt. The source is \
                 either very slow or refusing us."
            }
            Self::Internal => {
                "Something went wrong on our side, on every attempt. Send us \
                 the job id."
            }
            other => other.message(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobFailure {
    pub kind: JobFailureKind,
    pub message: String,
    pub retryable: bool,
}

impl From<JobFailureKind> for JobFailure {
    fn from(kind: JobFailureKind) -> Self {
        Self {
            message: kind.message().to_string(),
            retryable: kind.is_retryable(),
            kind,
        }
    }
}

/// A rendered export sitting ready for download.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobExport {
    pub format: ExportFormat,
    pub download_url: String,
    pub bytes: i64,
}

/// One transcription job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub status: JobStatus,
    /// 0.0–1.0. Reported by the worker where it can be measured (chunks
    /// finished / chunks total) and falls back to
    /// [`JobStatus::nominal_progress`].
    pub progress: f64,
    pub source: MediaSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaMetadata>,
    pub model: TranscriptionModel,
    #[serde(default)]
    pub features: Vec<TranscriptFeature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<Transcript>,
    #[serde(default)]
    pub exports: Vec<JobExport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<JobFailure>,
    /// Credits actually charged. Zero until the media duration is known.
    #[serde(default)]
    pub credits_charged: i64,
    /// Public share slug, when the transcript has been shared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share_slug: Option<String>,
    /// Parent job, when this is one video of a playlist or channel batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    /// Seconds of wall clock the job took, once terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

impl Job {
    /// Title for a list row, falling back to the source URL before the media
    /// metadata has been fetched.
    pub fn display_title(&self) -> &str {
        match &self.media {
            Some(m) if !m.title.is_empty() => &m.title,
            _ => self.source.display_url(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscribeResponse {
    pub job: Job,
    /// Credits the request is expected to cost. Present as soon as the media
    /// duration is known, which for a YouTube URL is before transcription
    /// starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_credits: Option<i64>,
    /// Estimated seconds until completion, from queue depth and the model's
    /// realtime factor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_secs: Option<f64>,
}

/// A playlist or channel submission, expanded into child jobs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Batch {
    pub id: String,
    pub source: MediaSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub total: usize,
    pub complete: usize,
    pub failed: usize,
    pub jobs: Vec<Job>,
    pub created_at: String,
}

impl Batch {
    pub fn is_finished(&self) -> bool {
        self.complete + self.failed >= self.total
    }

    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        (self.complete + self.failed) as f64 / self.total as f64
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobListQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<JobStatus>,
    /// Free-text match against the media title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobListResponse {
    pub jobs: Vec<Job>,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstimateRequest {
    pub source: MediaSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<TranscriptionModel>,
    #[serde(default)]
    pub features: Vec<TranscriptFeature>,
}

/// What a job will cost, quoted before it is started.
///
/// Exists so the dashboard never starts a job the account cannot finish: a
/// three-hour video on Tapestry with every feature on costs more than a whole
/// month of the entry plan, and finding that out half-way through is the worst
/// possible time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstimateResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaMetadata>,
    pub duration_secs: f64,
    pub model: TranscriptionModel,
    pub credits: i64,
    /// Human-readable media length, e.g. "1h 24m".
    pub duration_display: String,
    pub affordable: bool,
    pub remaining_credits: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_secs: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event: String,
    pub job: Job,
    pub sent_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_and_terminal_partition_every_status() {
        for status in [
            JobStatus::Queued,
            JobStatus::Downloading,
            JobStatus::Transcribing,
            JobStatus::Enriching,
            JobStatus::Rendering,
            JobStatus::Complete,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ] {
            assert_ne!(status.is_terminal(), status.is_running(), "{status}");
        }
    }

    #[test]
    fn status_round_trips_through_its_wire_form() {
        for status in [
            JobStatus::Queued,
            JobStatus::Downloading,
            JobStatus::Transcribing,
            JobStatus::Enriching,
            JobStatus::Rendering,
            JobStatus::Complete,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ] {
            assert_eq!(status.to_string().parse::<JobStatus>(), Ok(status));
        }
        // Both spellings of the American past participle are accepted, since
        // clients write either.
        assert_eq!("canceled".parse::<JobStatus>(), Ok(JobStatus::Cancelled));
        assert_eq!("completed".parse::<JobStatus>(), Ok(JobStatus::Complete));
    }

    #[test]
    fn progress_never_goes_backwards_through_the_pipeline() {
        let ordered = [
            JobStatus::Queued,
            JobStatus::Downloading,
            JobStatus::Transcribing,
            JobStatus::Enriching,
            JobStatus::Rendering,
            JobStatus::Complete,
        ];
        for pair in ordered.windows(2) {
            assert!(
                pair[0].nominal_progress() < pair[1].nominal_progress(),
                "{} -> {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn only_running_out_of_credits_keeps_the_charge() {
        assert!(!JobFailureKind::InsufficientCredits.refunds_credits());
        for kind in [
            JobFailureKind::MediaUnavailable,
            JobFailureKind::MediaTooLong,
            JobFailureKind::UnsupportedSource,
            JobFailureKind::NoSpeech,
            JobFailureKind::TranscriptionFailed,
            JobFailureKind::Timeout,
            JobFailureKind::Internal,
        ] {
            assert!(kind.refunds_credits(), "{kind:?}");
        }

        // A private video is never going to work, so a client must not retry it.
        assert!(!JobFailureKind::MediaUnavailable.is_retryable());
        assert!(JobFailureKind::Timeout.is_retryable());
    }

    /// Every failure kind, so a new one cannot skip the checks below.
    const ALL_FAILURE_KINDS: &[JobFailureKind] = &[
        JobFailureKind::MediaUnavailable,
        JobFailureKind::MediaTooLong,
        JobFailureKind::UnsupportedSource,
        JobFailureKind::NoSpeech,
        JobFailureKind::InsufficientCredits,
        JobFailureKind::TranscriptionFailed,
        JobFailureKind::Timeout,
        JobFailureKind::Internal,
    ];

    #[test]
    fn only_a_retryable_kind_may_invite_a_retry() {
        // A job that has burned through its attempts reports retryable=false
        // while still carrying `message()`. When that text said "safe to
        // retry" next to a disabled button, the product looked broken rather
        // than honest, which is what `exhausted_message` exists to fix.
        for kind in ALL_FAILURE_KINDS {
            if !kind.is_retryable() {
                assert!(
                    !kind.message().to_lowercase().contains("retry"),
                    "{kind:?} is not retryable but its message invites one"
                );
            }
            assert!(
                !kind.exhausted_message().to_lowercase().contains("safe to retry"),
                "{kind:?} has given up but still calls a retry safe"
            );
        }
    }

    #[test]
    fn a_kind_that_never_retried_says_the_same_thing_either_way() {
        // Nothing was promised, so there is nothing to walk back.
        for kind in ALL_FAILURE_KINDS.iter().filter(|k| !k.is_retryable()) {
            assert_eq!(kind.message(), kind.exhausted_message(), "{kind:?}");
        }
    }

    #[test]
    fn giving_up_changes_the_wording_for_every_transient_kind() {
        for kind in ALL_FAILURE_KINDS.iter().filter(|k| k.is_retryable()) {
            assert_ne!(
                kind.message(),
                kind.exhausted_message(),
                "{kind:?} is retryable, so the two messages must differ"
            );
        }
    }

    #[test]
    fn an_empty_batch_is_finished_rather_than_stuck() {
        let batch = Batch {
            id: "b1".into(),
            source: MediaSource::YoutubePlaylist { url: "x".into() },
            title: None,
            total: 0,
            complete: 0,
            failed: 0,
            jobs: vec![],
            created_at: String::new(),
        };
        assert!(batch.is_finished());
        assert_eq!(batch.progress(), 1.0);
    }

    #[test]
    fn a_batch_counts_failures_towards_being_finished() {
        let batch = Batch {
            id: "b1".into(),
            source: MediaSource::YoutubePlaylist { url: "x".into() },
            title: None,
            total: 4,
            complete: 3,
            failed: 1,
            jobs: vec![],
            created_at: String::new(),
        };
        assert!(batch.is_finished());
        assert_eq!(batch.progress(), 1.0);
    }
}
