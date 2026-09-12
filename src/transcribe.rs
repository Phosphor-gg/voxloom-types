use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::str::FromStr;

/// Billing unit.
///
/// One credit is one second of *source media*, not of processing time. Billing
/// the wall clock would make the same video cost different amounts depending on
/// how loaded the GPU was, which is impossible to quote up front and impossible
/// for a caller to verify.
///
/// Seconds rather than minutes because a 40-second clip should not be billed as
/// a minute, and because the API bills fractional media exactly. Every
/// user-facing surface renders these as minutes via [`format_credits_as_time`];
/// the raw credit figure is an implementation detail of the ledger.
pub const CREDITS_PER_MEDIA_SECOND: i64 = 1;

pub const CREDITS_PER_MINUTE: i64 = 60 * CREDITS_PER_MEDIA_SECOND;

/// Ceiling-divide media seconds into billable credits.
///
/// One credit is one second of audio. This took a model multiplier before,
/// which meant a balance could not be stated as a length of audio without
/// naming a model first, and every place that forgot produced a figure that
/// was wrong by a factor of two.
pub fn credits_for_media(duration_secs: f64) -> i64 {
    let seconds = duration_secs.ceil().max(0.0) as i64;
    seconds * CREDITS_PER_MEDIA_SECOND
}

/// Render a number of seconds as human time, e.g. "1h 24m" or "3m 20s".
///
/// For media lengths, speaking time and ETAs: things that are already measured
/// in seconds. Sub-minute values keep their seconds, because "0m" reads as
/// nothing when there is still a clip's worth there.
pub fn format_duration(secs: i64) -> String {
    let total = secs.max(0);
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m", minutes)
    } else {
        format!("{}s", seconds)
    }
}

/// Render a *credit* balance as the media time it buys, e.g. "1h 24m".
///
/// A credit is a second, so this is [`format_duration`] with a floor. It stays
/// a separate function because the two arguments mean different things, and
/// conflating them is precisely the bug that shipped while credits still
/// carried a model multiplier.
pub fn format_credits_as_time(credits: i64) -> String {
    format_duration(credits.max(0) / CREDITS_PER_MEDIA_SECOND)
}

/// Minutes a credit balance is worth, rounded down. For "N minutes included"
/// copy on pricing cards.
pub fn credits_to_minutes(credits: i64) -> i64 {
    credits.max(0) / CREDITS_PER_MINUTE
}

pub fn minutes_to_credits(minutes: i64) -> i64 {
    minutes * CREDITS_PER_MINUTE
}

/// Hard limits on a single transcription job. Enforced server-side and
/// mirrored in the SDKs so callers fail before uploading.
pub const MAX_MEDIA_DURATION_SECS: f64 = 12.0 * 60.0 * 60.0;
pub const MAX_UPLOAD_BYTES: i64 = 5 * 1024 * 1024 * 1024;

/// Videos longer than this are split into parts and transcribed in parallel.
///
/// Ten minutes is the point where a single-pass transcribe stops feeling
/// instant; below it the split costs more in overhead than it saves.
pub const PARALLEL_SPLIT_THRESHOLD_SECS: f64 = 10.0 * 60.0;

/// Length of each parallel chunk. Chunks overlap by
/// [`CHUNK_OVERLAP_SECS`] so a word spanning a boundary is not lost;
/// the overlap is de-duplicated when the parts are stitched.
pub const CHUNK_LENGTH_SECS: f64 = 300.0;
pub const CHUNK_OVERLAP_SECS: f64 = 5.0;

/// Maximum videos accepted from one playlist or channel submission.
pub const MAX_BATCH_VIDEOS: usize = 25;

/// Free demo allowances, per rolling 7-day window.
///
/// Anonymous callers are tracked by a salted hash of their IP; signed-in
/// callers by user id, which is why the signed-in allowance can be six times
/// larger without becoming free unlimited transcription.
pub const DEMO_ANON_WEEKLY_SECS: i64 = 5 * 60;
pub const DEMO_AUTHED_WEEKLY_SECS: i64 = 30 * 60;

/// The transcription model.
///
/// One model, not a ladder. Three tiers meant every caller had to pick a
/// quality level before they knew what the audio was like, every price had to
/// be quoted per tier, and the same balance meant a different number of
/// minutes depending on a choice made later. The enum survives as a single
/// variant because the wire format names the model and a second one may exist
/// one day; nothing may branch on it.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionModel {
    /// Whisper-large-v3 class: punctuation, casing, 101 languages, with the
    /// diarisation and cleanup passes on top.
    #[default]
    Weave,
}

impl TranscriptionModel {
    pub fn all_models() -> Vec<TranscriptionModel> {
        vec![Self::Weave]
    }

    /// Kept distinct from [`all_models`] because callers used it to mean
    /// "everything except Auto". There is no Auto now, so the two agree.
    pub fn concrete_models() -> Vec<TranscriptionModel> {
        Self::all_models()
    }

    pub fn version(&self) -> &str {
        "1.0"
    }

    pub fn base_name(&self) -> &str {
        "Weave"
    }

    pub fn to_name(&self) -> String {
        format!("{} {}", self.base_name(), self.version())
    }

    pub fn description(&self) -> &str {
        "Punctuation, casing, speaker labels and 101 languages."
    }

    /// Roughly how many seconds of media this transcribes per second of wall
    /// clock on the production GPU. Queue-time estimates only.
    pub fn realtime_factor(&self) -> f64 {
        40.0
    }

    /// No per-model entitlement: there is nothing to gate, because there is
    /// nothing to choose. Kept so call sites that ask need not care.
    pub fn entitlement(&self) -> Option<crate::entitlements::Entitlement> {
        None
    }
}

impl Display for TranscriptionModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Weave => "weave",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for TranscriptionModel {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            // The retired tier names are accepted and mapped onto the one
            // model rather than rejected: a saved script or an older client
            // asking for a tier that no longer exists should still transcribe,
            // and it now gets the good model either way.
            "weave" | "auto" | "thread" | "tapestry" => Ok(Self::Weave),
            _ => Err(()),
        }
    }
}

impl From<&str> for TranscriptionModel {
    fn from(s: &str) -> Self {
        s.parse().unwrap_or(Self::Weave)
    }
}

impl From<String> for TranscriptionModel {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

/// What the caller wants back alongside the raw transcript.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptFeature {
    /// Label each segment with a speaker, detecting the speaker count.
    Diarization,
    /// Strip fillers, stutters and false starts without changing wording.
    Cleanup,
    /// Abstractive summary plus key points and pull quotes.
    Summary,
    /// Chapter markers with titles and timestamps.
    Chapters,
    /// Translate the transcript into [`TranscribeRequest::translate_to`].
    Translation,
    /// Per-word timestamps, not just per-segment.
    WordTimestamps,
}

impl TranscriptFeature {
    pub fn all() -> Vec<TranscriptFeature> {
        vec![
            Self::Diarization,
            Self::Cleanup,
            Self::Summary,
            Self::Chapters,
            Self::Translation,
            Self::WordTimestamps,
        ]
    }

    pub fn to_name(&self) -> &str {
        match self {
            Self::Diarization => "Speaker labels",
            Self::Cleanup => "AI cleanup",
            Self::Summary => "AI summary",
            Self::Chapters => "Chapters",
            Self::Translation => "Translation",
            Self::WordTimestamps => "Word timestamps",
        }
    }

    /// Extra credits per media second on top of the model's own rate.
    ///
    /// Each of these is a separate pass over the audio or the transcript, so
    /// each carries its own cost rather than being folded into the model rate —
    /// a caller who only wants raw text should not subsidise summarisation.
    pub fn surcharge_per_second(&self) -> i64 {
        match self {
            Self::Diarization => 1,
            Self::Cleanup => 1,
            Self::Summary => 1,
            Self::Chapters => 0,
            Self::Translation => 1,
            Self::WordTimestamps => 0,
        }
    }

    pub fn entitlement(&self) -> Option<crate::entitlements::Entitlement> {
        use crate::entitlements::Entitlement as E;
        match self {
            Self::Diarization => Some(E::SpeakerDiarization),
            Self::Cleanup => Some(E::AiCleanup),
            Self::Summary => Some(E::AiSummary),
            Self::Chapters => Some(E::AiSummary),
            Self::Translation => Some(E::Translation),
            Self::WordTimestamps => None,
        }
    }
}

impl Display for TranscriptFeature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Diarization => "diarization",
            Self::Cleanup => "cleanup",
            Self::Summary => "summary",
            Self::Chapters => "chapters",
            Self::Translation => "translation",
            Self::WordTimestamps => "word_timestamps",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for TranscriptFeature {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().replace('-', "_").as_str() {
            "diarization" | "diarisation" | "speakers" => Ok(Self::Diarization),
            "cleanup" => Ok(Self::Cleanup),
            "summary" => Ok(Self::Summary),
            "chapters" => Ok(Self::Chapters),
            "translation" | "translate" => Ok(Self::Translation),
            "word_timestamps" | "words" => Ok(Self::WordTimestamps),
            _ => Err(()),
        }
    }
}

/// Output formats a finished transcript can be rendered into.
///
/// Rendering happens from the stored transcript, never by re-transcribing, so
/// exporting a second format after the fact costs nothing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Txt,
    Srt,
    Vtt,
    Docx,
    Pdf,
    Md,
    Json,
    Csv,
    Tsv,
    /// YouTube chapter list, ready to paste into a description.
    Chapters,
}

impl ExportFormat {
    pub fn all() -> Vec<ExportFormat> {
        vec![
            Self::Txt,
            Self::Srt,
            Self::Vtt,
            Self::Docx,
            Self::Pdf,
            Self::Md,
            Self::Json,
            Self::Csv,
            Self::Tsv,
            Self::Chapters,
        ]
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Srt => "srt",
            Self::Vtt => "vtt",
            Self::Docx => "docx",
            Self::Pdf => "pdf",
            Self::Md => "md",
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Chapters => "txt",
        }
    }

    pub fn mime(&self) -> &'static str {
        match self {
            Self::Txt | Self::Chapters => "text/plain; charset=utf-8",
            Self::Srt => "application/x-subrip; charset=utf-8",
            Self::Vtt => "text/vtt; charset=utf-8",
            Self::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
            Self::Pdf => "application/pdf",
            Self::Md => "text/markdown; charset=utf-8",
            Self::Json => "application/json",
            Self::Csv => "text/csv; charset=utf-8",
            Self::Tsv => "text/tab-separated-values; charset=utf-8",
        }
    }

    pub fn to_name(&self) -> &'static str {
        match self {
            Self::Txt => "Plain text",
            Self::Srt => "SubRip (.srt)",
            Self::Vtt => "WebVTT (.vtt)",
            Self::Docx => "Word (.docx)",
            Self::Pdf => "PDF",
            Self::Md => "Markdown",
            Self::Json => "JSON",
            Self::Csv => "CSV",
            Self::Tsv => "TSV",
            Self::Chapters => "YouTube chapters",
        }
    }

    /// Whether the format carries timing information. Timed formats are the
    /// ones worth offering as subtitles.
    pub fn is_timed(&self) -> bool {
        matches!(self, Self::Srt | Self::Vtt | Self::Json | Self::Csv | Self::Tsv)
    }
}

impl Display for ExportFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.extension())
    }
}

impl FromStr for ExportFormat {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().trim_start_matches('.') {
            "txt" | "text" => Ok(Self::Txt),
            "srt" | "subrip" => Ok(Self::Srt),
            "vtt" | "webvtt" => Ok(Self::Vtt),
            "docx" | "word" => Ok(Self::Docx),
            "pdf" => Ok(Self::Pdf),
            "md" | "markdown" => Ok(Self::Md),
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            "tsv" => Ok(Self::Tsv),
            "chapters" => Ok(Self::Chapters),
            _ => Err(()),
        }
    }
}

/// Where the media came from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaSource {
    /// A single YouTube video, by URL or bare id.
    Youtube { url: String },
    /// A YouTube playlist. Expanded to at most [`MAX_BATCH_VIDEOS`] videos.
    YoutubePlaylist { url: String },
    /// A YouTube channel. Expanded to its most recent
    /// [`MAX_BATCH_VIDEOS`] uploads.
    YoutubeChannel { url: String },
    /// Any other direct media URL (podcast RSS enclosure, mp3, mp4, ...).
    Url { url: String },
    /// A file the caller uploaded, referenced by the id returned from the
    /// upload endpoint.
    Upload { upload_id: String },
}

impl MediaSource {
    pub fn display_url(&self) -> &str {
        match self {
            Self::Youtube { url }
            | Self::YoutubePlaylist { url }
            | Self::YoutubeChannel { url }
            | Self::Url { url } => url,
            Self::Upload { upload_id } => upload_id,
        }
    }

    /// Whether this source expands into several jobs rather than one.
    pub fn is_batch(&self) -> bool {
        matches!(self, Self::YoutubePlaylist { .. } | Self::YoutubeChannel { .. })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeRequest {
    pub source: MediaSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<TranscriptionModel>,
    /// Features to run. Each is billed on top of the model rate; see
    /// [`TranscriptFeature::surcharge_per_second`].
    #[serde(default)]
    pub features: Vec<TranscriptFeature>,
    /// BCP-47 tag of the spoken language. Detected when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Target language for [`TranscriptFeature::Translation`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translate_to: Option<String>,
    /// Words, names and jargon to bias the decoder towards. Raises accuracy
    /// sharply on domain terms the model has never seen.
    #[serde(default)]
    pub vocabulary: Vec<String>,
    /// Expected number of speakers, when known. Improves diarisation over
    /// letting the clusterer guess.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_count: Option<u32>,
    /// Formats to render as soon as the transcript lands. More can be
    /// exported later at no cost.
    #[serde(default)]
    pub formats: Vec<ExportFormat>,
    /// Create a public share link for the finished transcript.
    #[serde(default)]
    pub share: bool,
    /// Webhook to POST the finished job to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
}

impl TranscribeRequest {
    pub fn youtube(url: impl Into<String>) -> Self {
        Self {
            source: MediaSource::Youtube { url: url.into() },
            model: None,
            features: Vec::new(),
            language: None,
            translate_to: None,
            vocabulary: Vec::new(),
            speaker_count: None,
            formats: Vec::new(),
            share: false,
            webhook_url: None,
        }
    }

    /// Total credits this request costs for a given media duration.
    ///
    /// The base is one credit per second of audio. Features still carry their
    /// own per-second surcharge, because they are extra work on the same
    /// audio; the model no longer does, because there is only one.
    pub fn estimate_credits(&self, duration_secs: f64) -> i64 {
        let seconds = duration_secs.ceil().max(0.0) as i64;
        let mut per_second = CREDITS_PER_MEDIA_SECOND;
        for feature in &self.features {
            per_second += feature.surcharge_per_second();
        }
        seconds * per_second
    }
}

/// One timed span of transcript.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    /// Speaker id from diarisation, e.g. "speaker_0". Renamed by the caller
    /// via [`RenameSpeakerRequest`], which rewrites every segment at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    /// Mean decoder confidence for the span, 0.0–1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<TranscriptWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptWord {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Speaker {
    pub id: String,
    /// Display name. Defaults to "Speaker 1", "Speaker 2", ... until renamed.
    pub label: String,
    /// Total seconds this speaker holds the floor.
    pub speaking_secs: f64,
    pub segment_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chapter {
    pub start_ms: i64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptSummary {
    /// Two to four sentence abstract.
    pub abstract_text: String,
    pub key_points: Vec<String>,
    /// Verbatim lines worth quoting, with the timestamp they were said at.
    pub pull_quotes: Vec<PullQuote>,
    #[serde(default)]
    pub action_items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PullQuote {
    pub text: String,
    pub start_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaMetadata {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    pub duration_secs: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}

/// A finished transcript.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transcript {
    /// Full text, paragraphed. This is the `cleanup` output when that feature
    /// ran, and the raw decode otherwise.
    pub text: String,
    /// Raw decode, kept even when `cleanup` ran so the verbatim wording is
    /// never lost — a legal or research transcript needs the fillers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbatim_text: Option<String>,
    pub segments: Vec<TranscriptSegment>,
    #[serde(default)]
    pub speakers: Vec<Speaker>,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<TranscriptSummary>,
    /// Detected (or supplied) language of the speech, BCP-47.
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translated_to: Option<String>,
    /// Mean decoder confidence across the whole transcript, 0.0–1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    pub word_count: usize,
    pub model: TranscriptionModel,
    pub model_version: String,
}

impl Transcript {
    /// Reading time at 220 words per minute, rounded up to a minute.
    pub fn reading_minutes(&self) -> usize {
        ((self.word_count as f64 / 220.0).ceil() as usize).max(1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameSpeakerRequest {
    pub speaker_id: String,
    pub label: String,
}

/// What the upload endpoint hands back once a file is staged.
///
/// The caller then submits a normal transcription request with
/// `MediaSource::Upload { upload_id }`. Splitting it in two means the browser
/// can show the real media length and the exact price before anything is
/// charged, which a single "upload and transcribe" call cannot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UploadResponse {
    pub upload_id: String,
    /// Original filename, echoed back so the client can label the row.
    pub filename: String,
    pub duration_secs: f64,
    pub size_bytes: i64,
    /// Credits the recording will cost on the default model, so the client can
    /// show a price without a second round-trip.
    pub credits: i64,
    /// When the staged file is swept if no job consumes it, RFC 3339.
    pub expires_at: String,
}

/// A language the pipeline can transcribe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LanguageInfo {
    /// BCP-47 tag, e.g. "en", "pt-BR".
    pub code: String,
    pub name: String,
    /// Name in the language itself.
    pub native_name: String,
    /// Whether the pipeline can translate *into* this language, not only from
    /// it. Transcription coverage is wider than translation coverage.
    pub translation_target: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagesResponse {
    pub languages: Vec<LanguageInfo>,
}

/// The 99 languages the transcription models cover.
///
/// Held here rather than in the website or the inference service because the
/// pricing page, the docs, the language picker and the API all quote the same
/// list, and a list stated four times is a list that disagrees with itself.
pub fn supported_languages() -> Vec<LanguageInfo> {
    const ROWS: &[(&str, &str, &str, bool)] = &[
        ("en", "English", "English", true),
        ("es", "Spanish", "Espanol", true),
        ("fr", "French", "Francais", true),
        ("de", "German", "Deutsch", true),
        ("it", "Italian", "Italiano", true),
        ("pt", "Portuguese", "Portugues", true),
        ("pt-BR", "Portuguese (Brazil)", "Portugues (Brasil)", true),
        ("nl", "Dutch", "Nederlands", true),
        ("ru", "Russian", "Russkij", true),
        ("uk", "Ukrainian", "Ukrainska", true),
        ("pl", "Polish", "Polski", true),
        ("cs", "Czech", "Cestina", true),
        ("sk", "Slovak", "Slovencina", true),
        ("sl", "Slovenian", "Slovenscina", true),
        ("hr", "Croatian", "Hrvatski", true),
        ("sr", "Serbian", "Srpski", true),
        ("bs", "Bosnian", "Bosanski", true),
        ("mk", "Macedonian", "Makedonski", true),
        ("bg", "Bulgarian", "Balgarski", true),
        ("ro", "Romanian", "Romana", true),
        ("hu", "Hungarian", "Magyar", true),
        ("el", "Greek", "Ellinika", true),
        ("tr", "Turkish", "Turkce", true),
        ("ar", "Arabic", "al-Arabiyya", true),
        ("he", "Hebrew", "Ivrit", true),
        ("fa", "Persian", "Farsi", true),
        ("ur", "Urdu", "Urdu", true),
        ("hi", "Hindi", "Hindi", true),
        ("bn", "Bengali", "Bangla", true),
        ("pa", "Punjabi", "Panjabi", true),
        ("gu", "Gujarati", "Gujarati", true),
        ("mr", "Marathi", "Marathi", true),
        ("ta", "Tamil", "Tamil", true),
        ("te", "Telugu", "Telugu", true),
        ("kn", "Kannada", "Kannada", true),
        ("ml", "Malayalam", "Malayalam", true),
        ("si", "Sinhala", "Sinhala", true),
        ("ne", "Nepali", "Nepali", true),
        ("zh", "Chinese", "Zhongwen", true),
        ("yue", "Cantonese", "Gwongdungwaa", false),
        ("ja", "Japanese", "Nihongo", true),
        ("ko", "Korean", "Hangugeo", true),
        ("vi", "Vietnamese", "Tieng Viet", true),
        ("th", "Thai", "Thai", true),
        ("lo", "Lao", "Lao", false),
        ("km", "Khmer", "Khmer", false),
        ("my", "Burmese", "Bamar", false),
        ("id", "Indonesian", "Bahasa Indonesia", true),
        ("ms", "Malay", "Bahasa Melayu", true),
        ("tl", "Tagalog", "Tagalog", true),
        ("jv", "Javanese", "Basa Jawa", false),
        ("su", "Sundanese", "Basa Sunda", false),
        ("sv", "Swedish", "Svenska", true),
        ("no", "Norwegian", "Norsk", true),
        ("nn", "Norwegian Nynorsk", "Nynorsk", false),
        ("da", "Danish", "Dansk", true),
        ("fi", "Finnish", "Suomi", true),
        ("is", "Icelandic", "Islenska", true),
        ("et", "Estonian", "Eesti", true),
        ("lv", "Latvian", "Latviesu", true),
        ("lt", "Lithuanian", "Lietuviu", true),
        ("sq", "Albanian", "Shqip", true),
        ("hy", "Armenian", "Hayeren", true),
        ("ka", "Georgian", "Kartuli", true),
        ("az", "Azerbaijani", "Azerbaycan", true),
        ("kk", "Kazakh", "Qazaqsha", true),
        ("uz", "Uzbek", "Ozbekcha", true),
        ("tk", "Turkmen", "Turkmence", false),
        ("tt", "Tatar", "Tatarca", false),
        ("ba", "Bashkir", "Bashqort", false),
        ("mn", "Mongolian", "Mongol", true),
        ("bo", "Tibetan", "Bod-yig", false),
        ("af", "Afrikaans", "Afrikaans", true),
        ("sw", "Swahili", "Kiswahili", true),
        ("am", "Amharic", "Amarinya", true),
        ("ha", "Hausa", "Hausa", true),
        ("yo", "Yoruba", "Yoruba", true),
        ("sn", "Shona", "ChiShona", false),
        ("so", "Somali", "Soomaali", true),
        ("mg", "Malagasy", "Malagasy", false),
        ("ny", "Nyanja", "Chichewa", false),
        ("ln", "Lingala", "Lingala", false),
        ("mt", "Maltese", "Malti", true),
        ("cy", "Welsh", "Cymraeg", true),
        ("ga", "Irish", "Gaeilge", true),
        ("gd", "Scottish Gaelic", "Gaidhlig", false),
        ("br", "Breton", "Brezhoneg", false),
        ("eu", "Basque", "Euskara", true),
        ("ca", "Catalan", "Catala", true),
        ("gl", "Galician", "Galego", true),
        ("oc", "Occitan", "Occitan", false),
        ("la", "Latin", "Latina", true),
        ("be", "Belarusian", "Belaruskaya", true),
        ("yi", "Yiddish", "Yidish", true),
        ("fo", "Faroese", "Faroyskt", false),
        ("lb", "Luxembourgish", "Letzebuergesch", true),
        ("ht", "Haitian Creole", "Kreyol Ayisyen", true),
        ("as", "Assamese", "Asamiya", false),
        ("sd", "Sindhi", "Sindhi", false),
        ("mi", "Maori", "Te Reo Maori", true),
        ("haw", "Hawaiian", "Olelo Hawaii", false),
    ];

    ROWS.iter()
        .map(|(code, name, native, target)| LanguageInfo {
            code: (*code).to_string(),
            name: (*name).to_string(),
            native_name: (*native).to_string(),
            translation_target: *target,
        })
        .collect()
}

pub fn language_count() -> usize {
    supported_languages().len()
}

/// Extract a YouTube video id from any of the URL shapes YouTube serves.
///
/// Lives in the shared crate because the website validates the demo input
/// before sending it and the backend validates it again on arrival; two
/// implementations of this would disagree on some shape and reject a URL the
/// other accepted.
pub fn youtube_video_id(input: &str) -> Option<String> {
    let trimmed = input.trim();

    // A bare id.
    if trimmed.len() == 11 && trimmed.chars().all(is_youtube_id_char) {
        return Some(trimmed.to_string());
    }

    let without_scheme = trimmed
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .trim_start_matches("m.");

    let (host, rest) = without_scheme.split_once('/').unwrap_or((without_scheme, ""));

    let candidate = match host {
        "youtu.be" => rest.split(['?', '&', '#', '/']).next().unwrap_or(""),
        "youtube.com" | "youtube-nocookie.com" | "music.youtube.com" => {
            if let Some(tail) = rest.strip_prefix("watch") {
                tail.trim_start_matches('?')
                    .split('&')
                    .find_map(|pair| pair.strip_prefix("v="))
                    .unwrap_or("")
            } else if let Some(tail) = rest.strip_prefix("shorts/") {
                tail.split(['?', '&', '#', '/']).next().unwrap_or("")
            } else if let Some(tail) = rest.strip_prefix("embed/") {
                tail.split(['?', '&', '#', '/']).next().unwrap_or("")
            } else if let Some(tail) = rest.strip_prefix("live/") {
                tail.split(['?', '&', '#', '/']).next().unwrap_or("")
            } else if let Some(tail) = rest.strip_prefix("v/") {
                tail.split(['?', '&', '#', '/']).next().unwrap_or("")
            } else {
                ""
            }
        }
        _ => "",
    };

    if candidate.len() == 11 && candidate.chars().all(is_youtube_id_char) {
        Some(candidate.to_string())
    } else {
        None
    }
}

fn is_youtube_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// Extract a YouTube playlist id, if the URL carries one.
pub fn youtube_playlist_id(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let is_bare_playlist_id = (trimmed.starts_with("PL")
        || trimmed.starts_with("UU")
        || trimmed.starts_with("OLAK"))
        && trimmed.chars().all(is_youtube_id_char);
    if is_bare_playlist_id {
        return Some(trimmed.to_string());
    }
    let query = trimmed.split_once('?').map(|(_, q)| q)?;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("list="))
        .map(|id| id.split(['&', '#']).next().unwrap_or(id).to_string())
        .filter(|id| !id.is_empty() && id.chars().all(is_youtube_id_char))
}

/// Classify a pasted string into the source it describes.
pub fn classify_source(input: &str) -> Option<MediaSource> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if youtube_video_id(trimmed).is_some() {
        return Some(MediaSource::Youtube { url: trimmed.to_string() });
    }
    if youtube_playlist_id(trimmed).is_some() {
        return Some(MediaSource::YoutubePlaylist { url: trimmed.to_string() });
    }
    let lower = trimmed.to_lowercase();
    if lower.contains("youtube.com/@")
        || lower.contains("youtube.com/c/")
        || lower.contains("youtube.com/channel/")
        || lower.contains("youtube.com/user/")
    {
        return Some(MediaSource::YoutubeChannel { url: trimmed.to_string() });
    }
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(MediaSource::Url { url: trimmed.to_string() });
    }
    None
}

/// Format milliseconds as an SRT timestamp (`HH:MM:SS,mmm`).
pub fn format_srt_timestamp(ms: i64) -> String {
    let ms = ms.max(0);
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) / 1000,
        ms % 1000
    )
}

/// Format milliseconds as a WebVTT timestamp (`HH:MM:SS.mmm`).
pub fn format_vtt_timestamp(ms: i64) -> String {
    format_srt_timestamp(ms).replace(',', ".")
}

/// Format milliseconds as a YouTube chapter timestamp: `M:SS` under an hour,
/// `H:MM:SS` at or over one. YouTube only links a chapter when the first one
/// starts at 0:00 and the format matches this exactly.
pub fn format_chapter_timestamp(ms: i64) -> String {
    let total = ms.max(0) / 1000;
    let hours = total / 3600;
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, (total % 3600) / 60, total % 60)
    } else {
        format!("{}:{:02}", total / 60, total % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The documented wire format, pinned.
    ///
    /// The docs, the OpenAPI spec and five SDKs all quote these strings. A
    /// rename here is a breaking change for every one of them, so it has to
    /// fail a test rather than quietly ship.
    #[test]
    fn the_request_wire_format_matches_what_is_documented() {
        // `kind`, not `type`: the docs and SDKs build this object by hand.
        let source = MediaSource::Youtube {
            url: "https://youtu.be/x".to_string(),
        };
        let json = serde_json::to_value(&source).unwrap();
        assert_eq!(json["kind"], "youtube");
        assert_eq!(json["url"], "https://youtu.be/x");

        for (source, kind) in [
            (
                MediaSource::YoutubePlaylist { url: "p".into() },
                "youtube_playlist",
            ),
            (
                MediaSource::YoutubeChannel { url: "c".into() },
                "youtube_channel",
            ),
            (MediaSource::Url { url: "u".into() }, "url"),
            (
                MediaSource::Upload {
                    upload_id: "i".into(),
                },
                "upload",
            ),
        ] {
            assert_eq!(serde_json::to_value(&source).unwrap()["kind"], kind);
        }
    }

    #[test]
    fn feature_model_and_format_names_are_pinned() {
        for (feature, wire) in [
            (TranscriptFeature::Diarization, "diarization"),
            (TranscriptFeature::Cleanup, "cleanup"),
            (TranscriptFeature::Summary, "summary"),
            (TranscriptFeature::Chapters, "chapters"),
            (TranscriptFeature::Translation, "translation"),
            (TranscriptFeature::WordTimestamps, "word_timestamps"),
        ] {
            assert_eq!(serde_json::to_value(feature).unwrap(), wire);
        }

        assert_eq!(
            serde_json::to_value(TranscriptionModel::Weave).unwrap(),
            "weave"
        );

        for (format, wire) in [
            (ExportFormat::Txt, "txt"),
            (ExportFormat::Srt, "srt"),
            (ExportFormat::Vtt, "vtt"),
            (ExportFormat::Docx, "docx"),
            (ExportFormat::Pdf, "pdf"),
            (ExportFormat::Md, "md"),
            (ExportFormat::Json, "json"),
            (ExportFormat::Csv, "csv"),
            (ExportFormat::Tsv, "tsv"),
            (ExportFormat::Chapters, "chapters"),
        ] {
            assert_eq!(serde_json::to_value(format).unwrap(), wire);
            // The path parameter accepts what the response advertises.
            assert_eq!(wire.parse::<ExportFormat>().unwrap(), format);
        }
    }

    #[test]
    fn a_balance_reads_the_same_as_the_plan_card_promises() {
        // The header formats a balance with format_credits_as_time and the
        // plan cards use credits_to_standard_minutes. If those disagree, a
        // customer sees one number on the pricing page and a different one in
        // the dashboard for the same allowance.
        for minutes in [10_i64, 300, 1_200, 6_000] {
            let credits = crate::pricing::standard_minutes_to_credits(minutes);
            assert_eq!(
                crate::pricing::credits_to_standard_minutes(credits),
                minutes,
                "round trip broke at {minutes} minutes"
            );

            let hours = minutes / 60;
            let expected = if hours > 0 {
                format!("{}h {}m", hours, minutes % 60)
            } else {
                format!("{}m", minutes)
            };
            assert_eq!(format_credits_as_time(credits), expected);
        }
    }

    #[test]
    fn a_media_length_is_not_run_through_the_credit_rate() {
        // 90 seconds of audio is 1m 30s of audio whatever it costs.
        assert_eq!(format_duration(90), "1m");
        assert_eq!(format_duration(3_661), "1h 1m");
        assert_eq!(format_duration(42), "42s");
        assert_eq!(format_duration(-5), "0s");
    }

    #[test]
    fn one_standard_minute_of_credits_reads_as_one_minute() {
        let one_minute = credits_for_media(60.0);
        assert_eq!(format_credits_as_time(one_minute), "1m");
    }

    #[test]
    fn video_id_parses_every_youtube_url_shape() {
        let expected = "dQw4w9WgXcQ";
        for input in [
            "dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/watch?v=dQw4w9WgXcQ&t=42s",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ?t=42",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube.com/embed/dQw4w9WgXcQ",
            "https://www.youtube.com/live/dQw4w9WgXcQ",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ",
            "  https://www.youtube.com/watch?v=dQw4w9WgXcQ  ",
        ] {
            assert_eq!(youtube_video_id(input).as_deref(), Some(expected), "{input}");
        }

        // The `v` parameter is not always first.
        assert_eq!(
            youtube_video_id("https://www.youtube.com/watch?t=1&v=dQw4w9WgXcQ").as_deref(),
            Some(expected)
        );
    }

    #[test]
    fn video_id_rejects_non_videos() {
        for input in [
            "",
            "not a url",
            "https://example.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=tooshort",
            "https://www.youtube.com/@somechannel",
        ] {
            assert!(youtube_video_id(input).is_none(), "{input}");
        }
    }

    #[test]
    fn playlists_and_channels_are_told_apart_from_videos() {
        assert!(matches!(
            classify_source("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            Some(MediaSource::Youtube { .. })
        ));
        assert!(matches!(
            classify_source("https://www.youtube.com/playlist?list=PLabcdefghijklmnop"),
            Some(MediaSource::YoutubePlaylist { .. })
        ));
        assert!(matches!(
            classify_source("https://www.youtube.com/@veritasium"),
            Some(MediaSource::YoutubeChannel { .. })
        ));
        assert!(matches!(
            classify_source("https://example.com/episode-12.mp3"),
            Some(MediaSource::Url { .. })
        ));
        assert!(classify_source("   ").is_none());

        // A watch URL that also carries ?list= is still one video: the user
        // clicked a video inside a playlist, they did not ask for the playlist.
        assert!(matches!(
            classify_source("https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLabcdefghij"),
            Some(MediaSource::Youtube { .. })
        ));
    }

    #[test]
    fn the_retired_tier_names_still_parse() {
        // A saved script or an older SDK may still send "thread" or
        // "tapestry". Rejecting those would fail a request that we can serve
        // perfectly well, so they map onto the one model.
        for name in ["weave", "auto", "thread", "tapestry", "WEAVE", "Tapestry"] {
            assert_eq!(
                name.parse::<TranscriptionModel>(),
                Ok(TranscriptionModel::Weave),
                "{name} should parse"
            );
        }
        assert!("gpt-4".parse::<TranscriptionModel>().is_err());
    }

    #[test]
    fn a_credit_is_exactly_a_second_of_audio() {
        // The whole point of dropping the tiers: a balance is a length of
        // audio, with no model to name first.
        assert_eq!(credits_for_media(1.0), CREDITS_PER_MEDIA_SECOND);
        assert_eq!(credits_for_media(60.0), 60 * CREDITS_PER_MEDIA_SECOND);
        // Part-seconds round up, so a 40 second clip is never billed as a
        // minute and never billed as 39 seconds either.
        assert_eq!(credits_for_media(39.2), 40 * CREDITS_PER_MEDIA_SECOND);
        assert_eq!(credits_for_media(0.0), 0);
        assert_eq!(credits_for_media(-3.0), 0);
    }

    #[test]
    fn feature_surcharges_are_added_to_the_base_rate() {
        let mut req = TranscribeRequest::youtube("dQw4w9WgXcQ");
        // A bare minute is a minute's worth of credits.
        assert_eq!(req.estimate_credits(60.0), 60);

        // Diarisation and summary each add 1 per second: 60 * (1 + 1 + 1).
        req.features = vec![TranscriptFeature::Diarization, TranscriptFeature::Summary];
        assert_eq!(req.estimate_credits(60.0), 180);

        // Chapters ride along with the summary pass and cost nothing extra.
        req.features.push(TranscriptFeature::Chapters);
        assert_eq!(req.estimate_credits(60.0), 180);
    }

    #[test]
    fn partial_seconds_round_up_so_media_is_never_billed_short() {
        assert_eq!(credits_for_media(0.2), 1);
        assert_eq!(credits_for_media(59.1), 60);
        assert_eq!(credits_for_media(0.0), 0);
    }

    #[test]
    fn balances_render_as_time_not_credits() {
        // A credit is a second, so these read straight across. They did not
        // before: 90 credits was 45 seconds when the standard rate was two
        // credits a second.
        assert_eq!(format_credits_as_time(0), "0s");
        assert_eq!(format_credits_as_time(90), "1m");
        assert_eq!(format_credits_as_time(120), "2m");
        assert_eq!(format_credits_as_time(3_600), "1h 0m");
        assert_eq!(format_credits_as_time(5_040), "1h 24m");
        // Negative balances (overdraft) read as empty, not as a negative time.
        assert_eq!(format_credits_as_time(-100), "0s");
    }

    #[test]
    fn chapter_timestamps_match_the_format_youtube_links() {
        assert_eq!(format_chapter_timestamp(0), "0:00");
        assert_eq!(format_chapter_timestamp(65_000), "1:05");
        assert_eq!(format_chapter_timestamp(3_725_000), "1:02:05");
        assert_eq!(format_srt_timestamp(3_725_123), "01:02:05,123");
        assert_eq!(format_vtt_timestamp(3_725_123), "01:02:05.123");
    }

    #[test]
    fn the_advertised_language_count_is_the_list_length() {
        let langs = supported_languages();
        assert_eq!(langs.len(), language_count());
        // The marketing copy, the docs and the language picker all quote this
        // number, so a change here is a change to published claims.
        assert_eq!(
            language_count(),
            101,
            "the advertised language count changed; update the site copy and docs too"
        );

        // Codes are unique; a duplicate would render twice in the picker.
        let mut codes: Vec<&str> = langs.iter().map(|l| l.code.as_str()).collect();
        codes.sort_unstable();
        let before = codes.len();
        codes.dedup();
        assert_eq!(before, codes.len());
    }
}
