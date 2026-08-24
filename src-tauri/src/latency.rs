//! Where the time actually goes, from a spoken word to scripture on a wall.
//!
//! Single responsibility: stamp one named instant per pipeline stage, and turn
//! the stamps into the five numbers a field test needs. It measures; it never
//! changes what the pipeline does.
//!
//! ## Why this exists
//!
//! Relay has been fast in every number it measured and slow in the one an
//! operator lives with. The last field test recorded a **0 ms mean backlog** and
//! a **778 ms decode median** and concluded the machine was keeping up — both
//! true, and neither of them is the question. "Keeping up" says the worker is
//! not falling further behind; it says nothing about how far behind it already
//! is. A pipeline that is exactly one decode, one cadence step and one
//! corroboration pass behind the preacher has a zero backlog forever, and the
//! transcript still lands three seconds after the words.
//!
//! Every stage below was, until now, invisible in isolation. When the transcript
//! felt late the only available diagnosis was "STT is slow", so that is what got
//! optimised — twice — while the cadence gate in front of the decoder was
//! costing more than the decoder.
//!
//! ## One clock
//!
//! Every stamp is monotonic microseconds since process start (`now_us`). Not the
//! audio clock: the STT worker drains its backlog per decode, so audio time
//! advances in jumps and two stamps taken a second apart can be ten audio
//! seconds apart (see `main::router_clock_ms`, which learned this the hard way).
//! Not epoch time either — an NTP step mid-service would rewrite the report.
//!
//! The frontend has neither clock. It reports `Date.now()`, and `EPOCH_ANCHOR`
//! converts: one epoch/monotonic pair sampled at start-up, subtracted. That is
//! exact on a machine whose wall clock does not step, and the arrival stamp
//! taken when the mark reaches Rust bounds the error either way — the gap
//! between them IS the IPC hop, which is reported rather than hidden.
//!
//! ## What a trace is
//!
//! One DECODE PASS. That is the unit the pipeline actually works in: a pass
//! consumes the audio that has arrived since the last one, produces one
//! transcript, and that transcript is what detection reads. A pass that names a
//! reference carries on through the router and out to the wall, so a single
//! trace spans microphone to projector.
//!
//! Traces are cheap and lossy on purpose. The ring holds the most recent few
//! hundred for inspection; the histograms hold every sample for the whole
//! service, which is what "did it get worse over three hours" needs.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// How many complete traces are kept for inspection. The histograms below keep
/// every sample; this is only the detail view, and a field tester looking at
/// detail is looking at the last minute or two.
const RING: usize = 256;
/// Histogram resolution. One millisecond, because every target in the acceptance
/// criteria is stated in whole milliseconds and a percentile should not be an
/// artefact of the bucket width. 8000 buckets is 32 KB per metric — nothing, for
/// a desktop app, and it buys exact-to-the-millisecond answers over a whole
/// service instead of a rolling window's worth.
const BUCKET_US: u64 = 1_000;
/// Histogram ceiling. Anything past 8 s is "investigate", and the exact value
/// stops mattering — but it is still counted, and `over` reports it, because a
/// silently-dropped tail is how a P95 flatters itself.
const BUCKETS: usize = 8_000;
/// Width of a drift bucket. The question "is it getting worse" is answered over
/// minutes, not seconds.
const DRIFT_BUCKET_US: u64 = 60 * 1_000_000;
/// How many drift buckets are kept — four hours, comfortably longer than any
/// service.
const DRIFT_BUCKETS: usize = 240;
/// How long an open trace waits for stages that may never arrive.
///
/// Most decode passes name no reference, so `ReferenceDetected` and everything
/// after it never happen for them. Something has to decide those traces are done.
///
/// The obvious answer — close the trace the moment detection finds nothing — is
/// wrong, and wrong in a way that would have been invisible: the console's render
/// mark comes back over the IPC bridge a few milliseconds LATER, so closing on the
/// detection result would drop it, and `audio_to_visible_transcript` (the number
/// the whole exercise is about) would have almost no samples while looking
/// perfectly healthy at n=3.
///
/// So a trace is retired on a deadline instead. Ten seconds is far longer than any
/// path being measured — the worst end-to-end target is 1.5 s — and generous
/// enough that a slow pass is recorded as slow rather than discarded for being
/// slow, which would flatter every percentile here.
const STALE_US: u64 = 10 * 1_000_000;

/// Monotonic microseconds since the first call. THE clock for every stamp here.
pub fn now_us() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_micros() as u64
}

/// (epoch_us, mono_us) sampled once, so a frontend `Date.now()` can be placed on
/// the monotonic timeline. Sampled lazily on first use rather than at start-up so
/// this module has no initialisation order to get wrong.
fn epoch_anchor() -> (u64, u64) {
    static ANCHOR: OnceLock<(u64, u64)> = OnceLock::new();
    *ANCHOR.get_or_init(|| {
        let mono = now_us();
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        (epoch, mono)
    })
}

/// Place a frontend epoch-millisecond stamp on the monotonic timeline.
///
/// Saturating on purpose: a wall clock that has stepped backwards since the
/// anchor would otherwise underflow into a nonsense 500-million-second latency,
/// and a clamped-to-zero stage reads as "instant", which the report can show is
/// wrong when the arrival stamp disagrees.
pub fn from_epoch_ms(epoch_ms: u64) -> u64 {
    let (epoch0, mono0) = epoch_anchor();
    mono0.saturating_add(epoch_ms.saturating_mul(1_000).saturating_sub(epoch0))
}

/// The stages, in the order the pipeline visits them.
///
/// Named exactly as the field-test specification names them, so a report can be
/// read against it without a translation table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The OLDEST audio still waiting to be transcribed reached the STT worker.
    /// Not the newest — see the STT worker, which explains why measuring from the
    /// freshest chunk in a batch describes the word that waited least.
    AudioReceived,
    /// The voice gate opened the utterance this pass belongs to.
    VoiceDetected,
    /// Whisper's blocking decode began.
    SttStarted,
    /// Rust emitted the transcript (`stt://transcript`).
    PartialTranscript,
    /// The console reported the transcript painted.
    TranscriptRendered,
    /// A reference was parsed out of that transcript.
    ReferenceDetected,
    /// The router said it may reach a wall.
    FireAuthorised,
    /// The content left the machine — event emitted, kiosk broadcast done.
    FireSent,
    /// An output page reported the verse painted.
    OutputRendered,
}

impl Stage {
    /// Every stage, for iteration in a fixed order.
    pub const ALL: [Stage; 9] = [
        Stage::AudioReceived,
        Stage::VoiceDetected,
        Stage::SttStarted,
        Stage::PartialTranscript,
        Stage::TranscriptRendered,
        Stage::ReferenceDetected,
        Stage::FireAuthorised,
        Stage::FireSent,
        Stage::OutputRendered,
    ];

    fn idx(self) -> usize {
        Stage::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// The wire name the frontend uses when reporting a mark.
    pub fn from_wire(s: &str) -> Option<Stage> {
        Some(match s {
            "transcript_rendered" => Stage::TranscriptRendered,
            "output_rendered" => Stage::OutputRendered,
            _ => return None,
        })
    }
}

/// The five spans the acceptance criteria are written in, plus the two the
/// pipeline needs to tell an STT problem from a routing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Audio in the worker → transcript emitted by Rust.
    AudioToPartial,
    /// Audio in the worker → transcript painted on the console.
    AudioToVisible,
    /// Transcript emitted → a reference parsed out of it.
    TranscriptToReference,
    /// Reference parsed → content left the machine.
    ReferenceToFire,
    /// Voice gate opened → verse painted on an output.
    SpeechToScripture,
    /// How long whisper's blocking decode took.
    Decode,
    /// Wall time between one transcript and the next, WITHIN an utterance — the
    /// cadence, which is what an operator perceives as "is it keeping up". The gap
    /// after a closed utterance is a person not talking, and is excluded.
    Cadence,
}

impl Metric {
    pub const ALL: [Metric; 7] = [
        Metric::AudioToPartial,
        Metric::AudioToVisible,
        Metric::TranscriptToReference,
        Metric::ReferenceToFire,
        Metric::SpeechToScripture,
        Metric::Decode,
        Metric::Cadence,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Metric::AudioToPartial => "audio_to_partial_transcript",
            Metric::AudioToVisible => "audio_to_visible_transcript",
            Metric::TranscriptToReference => "transcript_to_reference_detection",
            Metric::ReferenceToFire => "reference_detection_to_fire",
            Metric::SpeechToScripture => "end_to_end_speech_to_scripture",
            Metric::Decode => "stt_decode",
            Metric::Cadence => "transcript_cadence",
        }
    }

    /// The span, from a trace's stamps, or None when this trace never reached
    /// both ends of it. A trace that named no reference has no
    /// `TranscriptToReference`, and that is an absence, never a zero.
    fn span(self, t: &Trace) -> Option<u64> {
        let (a, b) = match self {
            Metric::AudioToPartial => (Stage::AudioReceived, Stage::PartialTranscript),
            Metric::AudioToVisible => (Stage::AudioReceived, Stage::TranscriptRendered),
            Metric::TranscriptToReference => (Stage::PartialTranscript, Stage::ReferenceDetected),
            Metric::ReferenceToFire => (Stage::ReferenceDetected, Stage::FireSent),
            Metric::SpeechToScripture => (Stage::VoiceDetected, Stage::OutputRendered),
            // Not a span between two stages — recorded directly.
            Metric::Decode | Metric::Cadence => return None,
        };
        let from = t.stamps[a.idx()]?;
        let to = t.stamps[b.idx()]?;
        Some(to.saturating_sub(from))
    }
}

/// One decode pass, and everything that happened because of it.
#[derive(Debug, Clone, Serialize)]
pub struct Trace {
    pub id: u64,
    /// Monotonic microsecond stamps, indexed by `Stage::idx`.
    pub stamps: [Option<u64>; 9],
    /// Was the transcript a closed utterance?
    pub is_final: bool,
    /// How many audio chunks this pass swallowed. A number well above one means
    /// the worker caught up on a backlog in a single decode, which is the loop
    /// working as designed — and a number that keeps climbing means it is not.
    pub drained: usize,
    /// Milliseconds of audio in the decoded window.
    pub window_ms: u64,
    /// Whisper's blocking decode, microseconds.
    pub decode_us: u64,
    /// How long the frontend's mark took to travel back over the IPC bridge —
    /// arrival-in-Rust minus the frontend's own stamp. Reported, not corrected:
    /// it is a real cost on the path and it is the one number that says whether a
    /// slow-looking render is the webview or the bridge.
    pub ipc_return_us: Option<u64>,
}

impl Trace {
    fn new(id: u64) -> Self {
        Trace {
            id,
            stamps: [None; 9],
            is_final: false,
            drained: 0,
            window_ms: 0,
            decode_us: 0,
            ipc_return_us: None,
        }
    }

    /// First write wins. A stage is reached once per pass; a second stamp is a
    /// caller bug or a duplicate frontend mark, and taking the later of the two
    /// would silently inflate every span that ends there.
    fn stamp(&mut self, stage: Stage, at: u64) {
        let slot = &mut self.stamps[stage.idx()];
        if slot.is_none() {
            *slot = Some(at);
        }
    }

    pub fn at(&self, stage: Stage) -> Option<u64> {
        self.stamps[stage.idx()]
    }
}

/// A fixed-bucket histogram. Every sample for the whole service, in 6 KB.
///
/// A ring of recent samples would have been simpler and would have answered the
/// wrong question: a service is ninety minutes long and the acceptance criteria
/// ask about all of it.
#[derive(Debug, Clone)]
pub struct Hist {
    buckets: Vec<u32>,
    /// Samples past the ceiling. Counted, and reported — a percentile computed
    /// over a silently-truncated tail is a flattering lie.
    over: u64,
    count: u64,
    sum: u64,
    max: u64,
}

impl Default for Hist {
    fn default() -> Self {
        Hist {
            buckets: vec![0; BUCKETS],
            over: 0,
            count: 0,
            sum: 0,
            max: 0,
        }
    }
}

impl Hist {
    pub fn add(&mut self, us: u64) {
        self.count += 1;
        self.sum += us;
        self.max = self.max.max(us);
        let b = (us / BUCKET_US) as usize;
        match self.buckets.get_mut(b) {
            Some(slot) => *slot += 1,
            None => self.over += 1,
        }
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    /// The value at `q` (0.0–1.0), in microseconds.
    ///
    /// Returns the EXCLUSIVE TOP of the containing bucket, so the answer is an
    /// upper bound rather than a midpoint guess: a P95 that reads 300 ms means
    /// "95% were under 300 ms", which is the claim the acceptance criteria make.
    /// It therefore reads one bucket — one millisecond — pessimistic, and never
    /// flattering, which is the direction a performance number should err in.
    /// Samples past the ceiling report the observed max, which is exact.
    pub fn quantile(&self, q: f64) -> Option<u64> {
        if self.count == 0 {
            return None;
        }
        let target = ((self.count as f64) * q).ceil().max(1.0) as u64;
        let mut seen = 0u64;
        for (i, n) in self.buckets.iter().enumerate() {
            seen += *n as u64;
            if seen >= target {
                return Some(((i as u64) + 1) * BUCKET_US);
            }
        }
        Some(self.max)
    }

    pub fn mean(&self) -> Option<u64> {
        (self.count > 0).then(|| self.sum / self.count)
    }
}

/// Per-minute totals for one metric. Enough to answer "did it get worse",
/// which is a question about a trend and not about a percentile.
#[derive(Debug, Clone)]
struct Drift {
    count: [u32; DRIFT_BUCKETS],
    sum: [u64; DRIFT_BUCKETS],
    max: [u64; DRIFT_BUCKETS],
}

impl Default for Drift {
    fn default() -> Self {
        Drift {
            count: [0; DRIFT_BUCKETS],
            sum: [0; DRIFT_BUCKETS],
            max: [0; DRIFT_BUCKETS],
        }
    }
}

impl Drift {
    fn add(&mut self, at_us: u64, us: u64) {
        let b = (at_us / DRIFT_BUCKET_US) as usize;
        if b < DRIFT_BUCKETS {
            self.count[b] += 1;
            self.sum[b] += us;
            self.max[b] = self.max[b].max(us);
        }
    }
}

#[derive(Default)]
struct Inner {
    ring: VecDeque<Trace>,
    /// Traces still open — a pass whose transcript has been emitted but whose
    /// render mark, detection or fire has not happened yet. Bounded by RING:
    /// an entry nothing ever reports would otherwise leak for the whole service.
    open: VecDeque<Trace>,
    hists: Vec<Hist>,
    drifts: Vec<Drift>,
    /// When the previous pass emitted its transcript, for the cadence metric.
    last_partial_us: Option<u64>,
    /// Whether that previous transcript CLOSED an utterance. A gap after a final
    /// is silence — the speaker stopped — and timing it would report the pauses
    /// in a sermon as pipeline latency. Measured on five minutes of continuous
    /// speech the medians agreed and the worst case did not: 201 ms against
    /// 8604 ms, the latter being a pause between readings and nothing else.
    last_partial_was_final: bool,
}

/// The recorder. One per process; `reset()` exists for tests.
pub struct Recorder {
    inner: Mutex<Inner>,
    next_id: AtomicU64,
    enabled: AtomicBool,
}

fn recorder() -> &'static Recorder {
    static R: OnceLock<Recorder> = OnceLock::new();
    R.get_or_init(|| Recorder {
        inner: Mutex::new(Inner {
            ring: VecDeque::with_capacity(RING),
            open: VecDeque::with_capacity(RING),
            hists: (0..Metric::ALL.len()).map(|_| Hist::default()).collect(),
            drifts: (0..Metric::ALL.len()).map(|_| Drift::default()).collect(),
            last_partial_us: None,
            last_partial_was_final: false,
        }),
        next_id: AtomicU64::new(1),
        // On by default. The whole point is that a field tester on a packaged
        // build can read the numbers without a special build — and the cost is a
        // handful of integer stamps per decode against a decode measured in
        // hundreds of milliseconds. Instrumentation nobody can switch on is
        // instrumentation nobody uses.
        enabled: AtomicBool::new(true),
    })
}

/// Count a partial transcript shed because the detection queue was full.
///
/// Shedding is the correct response to back-pressure on a partial — the same
/// window is decoded again a moment later — but it must never be silent. A
/// pipeline that quietly drops half its work looks identical, in every latency
/// number, to one that is keeping up.
pub fn note_dropped_partial() {
    DROPPED_PARTIALS.fetch_add(1, Ordering::Relaxed);
}

static DROPPED_PARTIALS: AtomicU64 = AtomicU64::new(0);

pub fn set_enabled(on: bool) {
    recorder().enabled.store(on, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    recorder().enabled.load(Ordering::Relaxed)
}

/// Open a trace for a decode pass that is about to start.
///
/// `audio_received_us` is when the newest audio in the window reached the worker
/// and `voice_us` is when the gate opened this utterance — both already known by
/// the caller, both from before this call, which is why they are passed in
/// rather than sampled here.
pub fn begin_pass(audio_received_us: u64, voice_us: Option<u64>) -> u64 {
    let r = recorder();
    let id = r.next_id.fetch_add(1, Ordering::Relaxed);
    if !r.enabled.load(Ordering::Relaxed) {
        return id;
    }
    let mut t = Trace::new(id);
    t.stamp(Stage::AudioReceived, audio_received_us);
    if let Some(v) = voice_us {
        t.stamp(Stage::VoiceDetected, v);
    }
    t.stamp(Stage::SttStarted, now_us());
    if let Ok(mut g) = r.inner.lock() {
        expire_stale(&mut g, now_us());
        push_open(&mut g, t);
    }
    id
}

/// Retire open traces that will never be completed. Called on every new pass, so
/// the report is live rather than lagging the ring by several minutes.
///
/// `open` is in creation order, so this stops at the first trace young enough to
/// still be waiting on something — it is a prefix scan, not a sweep of the whole
/// deque.
fn expire_stale(g: &mut Inner, now: u64) {
    while let Some(front) = g.open.front() {
        let born = front.at(Stage::SttStarted).unwrap_or(0);
        if now.saturating_sub(born) < STALE_US {
            return;
        }
        if let Some(t) = g.open.pop_front() {
            retire(g, t);
        }
    }
}

fn push_open(g: &mut Inner, t: Trace) {
    if g.open.len() >= RING {
        // The oldest open trace is one nothing is going to complete — a pass whose
        // transcript named no reference and whose render mark never arrived. Retire
        // it into the ring so its partial spans still count, rather than dropping it.
        if let Some(old) = g.open.pop_front() {
            retire(g, old);
        }
    }
    g.open.push_back(t);
}

/// Fold a finished trace's spans into the histograms and park it in the ring.
fn retire(g: &mut Inner, t: Trace) {
    for (i, m) in Metric::ALL.iter().enumerate() {
        let sample = match m {
            Metric::Decode => (t.decode_us > 0).then_some(t.decode_us),
            Metric::Cadence => None, // recorded at emit time, not here
            other => other.span(&t),
        };
        if let Some(us) = sample {
            g.hists[i].add(us);
            let at = t.at(Stage::SttStarted).unwrap_or(0);
            g.drifts[i].add(at, us);
        }
    }
    if g.ring.len() >= RING {
        g.ring.pop_front();
    }
    g.ring.push_back(t);
}

/// Apply `f` to the open trace with this id.
fn with_open<F: FnOnce(&mut Trace)>(id: u64, f: F) {
    let r = recorder();
    if !r.enabled.load(Ordering::Relaxed) {
        return;
    }
    if let Ok(mut g) = r.inner.lock() {
        if let Some(t) = g.open.iter_mut().find(|t| t.id == id) {
            f(t);
        }
    }
}

/// Stamp a stage on an open trace, now.
pub fn stamp(id: u64, stage: Stage) {
    let at = now_us();
    with_open(id, |t| t.stamp(stage, at));
}

/// Stamp a stage at a time already measured.
pub fn stamp_at(id: u64, stage: Stage, at_us: u64) {
    with_open(id, |t| t.stamp(stage, at_us));
}

/// The transcript for this pass has been emitted. Records the decode cost and the
/// cadence — the gap since the previous pass's transcript, which is the number an
/// operator reads as "is it keeping up".
pub fn transcript_emitted(id: u64, decode_us: u64, window_ms: u64, drained: usize, is_final: bool) {
    let r = recorder();
    if !r.enabled.load(Ordering::Relaxed) {
        return;
    }
    let at = now_us();
    if let Ok(mut g) = r.inner.lock() {
        // Only WITHIN an utterance. See `last_partial_was_final`.
        if let (Some(prev), false) = (g.last_partial_us, g.last_partial_was_final) {
            let gap = at.saturating_sub(prev);
            let i = Metric::Cadence as usize;
            g.hists[i].add(gap);
            g.drifts[i].add(at, gap);
        }
        g.last_partial_us = Some(at);
        g.last_partial_was_final = is_final;
        if let Some(t) = g.open.iter_mut().find(|t| t.id == id) {
            t.stamp(Stage::PartialTranscript, at);
            t.decode_us = decode_us;
            t.window_ms = window_ms;
            t.drained = drained;
            t.is_final = is_final;
        }
    }
}

/// A frontend mark. `at_epoch_ms` is the frontend's own clock; `arrived_us` is
/// when it reached Rust. Both are kept — their difference is the bridge.
pub fn frontend_mark(id: u64, stage: Stage, at_epoch_ms: u64) {
    let arrived = now_us();
    let converted = from_epoch_ms(at_epoch_ms);
    // A frontend stamp that lands after its own arrival, or before the decode
    // that produced it, is a clock that has stepped. Fall back to the arrival
    // stamp, which is always on the right timeline and merely pessimistic.
    let at = if converted > arrived {
        arrived
    } else {
        converted
    };
    with_open(id, |t| {
        t.stamp(stage, at);
        if t.ipc_return_us.is_none() {
            t.ipc_return_us = Some(arrived.saturating_sub(at));
        }
    });
    // An output render is the last stage there is: nothing else will arrive for
    // this trace, so close it now rather than waiting for the ring to evict it.
    if stage == Stage::OutputRendered {
        close(id);
    }
}

/// Retire a trace: no further stage will be reached.
pub fn close(id: u64) {
    let r = recorder();
    if !r.enabled.load(Ordering::Relaxed) {
        return;
    }
    if let Ok(mut g) = r.inner.lock() {
        if let Some(pos) = g.open.iter().position(|t| t.id == id) {
            if let Some(t) = g.open.remove(pos) {
                retire(&mut g, t);
            }
        }
    }
}

/// One metric, as a field test reads it.
#[derive(Debug, Clone, Serialize)]
pub struct MetricReport {
    pub metric: &'static str,
    pub samples: u64,
    pub mean_ms: Option<f64>,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub worst_ms: Option<f64>,
    /// Samples past the histogram ceiling. Non-zero means the tail is worse than
    /// the buckets can describe, and `worst_ms` is where to look.
    pub over_ceiling: u64,
    /// Mean per minute of the service, oldest first, trailing empty minutes
    /// trimmed. This is the answer to "does latency grow over a long service" —
    /// a rising line here is the finding, whatever the P50 says.
    pub per_minute_mean_ms: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Seconds of wall time this report covers.
    pub uptime_s: f64,
    pub enabled: bool,
    /// What the capture front-end adds BEFORE `audio_received_at`, in
    /// milliseconds. A chunk is this many milliseconds of audio and is assembled
    /// only once it is full, so a word lands somewhere in that window: 0 at the
    /// end of a chunk, a full chunk at the start of one, and about half of it on
    /// average. The device's own buffer sits underneath and is not measured here.
    ///
    /// Stated rather than folded in, because `audio_received_at` means what it
    /// says. Anyone comparing this report against a stopwatch held in a room has
    /// to add it, and `end_to_end_speech_to_scripture` already has.
    pub capture_front_end_ms: u64,
    pub metrics: Vec<MetricReport>,
    /// Transcript updates per second, over the whole session. The inverse of the
    /// cadence mean, stated directly because it is what the acceptance criteria
    /// ask for.
    pub transcript_updates_per_s: Option<f64>,
    /// The most recent traces, newest first, for reading a single pass end to end.
    pub recent: Vec<Trace>,
    /// Partial transcripts shed because the detection queue was full. Should be
    /// zero; a number that climbs during a service is the pipeline telling you
    /// detection has stopped keeping up with the decoder.
    pub dropped_partials: u64,
    /// Traces still waiting on a stage. A number that sits near the ring size
    /// means marks are not coming back — a console or output page that stopped
    /// reporting, not a pipeline that got slow.
    pub open_traces: usize,
}

fn us_to_ms(us: u64) -> f64 {
    us as f64 / 1000.0
}

pub fn report(recent_n: usize) -> Report {
    let r = recorder();
    let Ok(mut g) = r.inner.lock() else {
        return Report {
            uptime_s: 0.0,
            enabled: false,
            metrics: Vec::new(),
            capture_front_end_ms: crate::audio::CHUNK_MS as u64,
            transcript_updates_per_s: None,
            recent: Vec::new(),
            dropped_partials: DROPPED_PARTIALS.load(Ordering::Relaxed),
            open_traces: 0,
        };
    };
    expire_stale(&mut g, now_us());
    let metrics = Metric::ALL
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let h = &g.hists[i];
            let d = &g.drifts[i];
            let last = (0..DRIFT_BUCKETS).rev().find(|b| d.count[*b] > 0);
            let per_minute = match last {
                None => Vec::new(),
                Some(last) => (0..=last)
                    .map(|b| {
                        if d.count[b] == 0 {
                            0.0
                        } else {
                            us_to_ms(d.sum[b] / d.count[b] as u64)
                        }
                    })
                    .collect(),
            };
            MetricReport {
                metric: m.label(),
                samples: h.count(),
                mean_ms: h.mean().map(us_to_ms),
                p50_ms: h.quantile(0.50).map(us_to_ms),
                p95_ms: h.quantile(0.95).map(us_to_ms),
                worst_ms: (h.count() > 0).then(|| us_to_ms(h.max)),
                over_ceiling: h.over,
                per_minute_mean_ms: per_minute,
            }
        })
        .collect();
    let cadence = &g.hists[Metric::Cadence as usize];
    let ups = cadence
        .mean()
        .filter(|m| *m > 0)
        .map(|m| 1_000_000.0 / m as f64);
    let recent: Vec<Trace> = g.ring.iter().rev().take(recent_n).cloned().collect();
    Report {
        uptime_s: now_us() as f64 / 1_000_000.0,
        enabled: r.enabled.load(Ordering::Relaxed),
        capture_front_end_ms: crate::audio::CHUNK_MS as u64,
        metrics,
        transcript_updates_per_s: ups,
        recent,
        dropped_partials: DROPPED_PARTIALS.load(Ordering::Relaxed),
        open_traces: g.open.len(),
    }
}

/// Forget everything measured so far. For starting a clean field-test run, and
/// for keeping tests independent of each other.
pub fn reset() {
    let r = recorder();
    if let Ok(mut g) = r.inner.lock() {
        g.ring.clear();
        g.open.clear();
        g.hists = (0..Metric::ALL.len()).map(|_| Hist::default()).collect();
        g.drifts = (0..Metric::ALL.len()).map(|_| Drift::default()).collect();
        g.last_partial_us = None;
        g.last_partial_was_final = false;
    }
    DROPPED_PARTIALS.store(0, Ordering::Relaxed);
}

/// The recorder is a PROCESS-WIDE singleton, and `cargo test` runs tests in
/// parallel threads of one process. Any test that resets it, or that asserts on
/// counts it produced, has to hold this — otherwise a test in another module
/// clears the histograms halfway through an assertion and the failure is a
/// once-in-twenty flake with no relationship to the code under test.
///
/// Poisoning is recovered from rather than propagated: a panic in one test must
/// fail that test, not every subsequent one.
#[cfg(test)]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hold the shared recorder lock and start from a clean slate.
    fn guard() -> std::sync::MutexGuard<'static, ()> {
        let held = test_lock();
        reset();
        set_enabled(true);
        held
    }

    /// `Metric as usize` indexes the histogram vectors, and `Metric::ALL` orders
    /// the report. If those two ever disagree, every number in the report is
    /// silently attributed to the wrong metric — and it would still look like a
    /// plausible report.
    #[test]
    fn the_metric_discriminant_is_its_index_in_all() {
        for (i, m) in Metric::ALL.iter().enumerate() {
            assert_eq!(*m as usize, i, "{} is out of order", m.label());
        }
    }

    #[test]
    fn a_span_is_the_difference_between_two_stamps() {
        let _l = guard();
        let id = begin_pass(1_000, Some(500));
        stamp_at(id, Stage::PartialTranscript, 400_000);
        close(id);
        let r = report(4);
        let m = r
            .metrics
            .iter()
            .find(|m| m.metric == "audio_to_partial_transcript")
            .expect("metric");
        assert_eq!(m.samples, 1);
        // 400_000us - 1_000us = 399ms exactly, reported as the exclusive top of
        // the millisecond bucket holding it.
        assert_eq!(m.p50_ms, Some(400.0));
    }

    /// THE BUG THIS GUARDS. A stage that was never reached is an ABSENCE, and a
    /// report that counted it as zero would claim a pipeline stage costing nothing
    /// on every pass that never got there — which is most of them, because most
    /// windows contain no scripture reference at all. The P50 of "detection to
    /// fire" would then read 0 ms forever while a real fire took a second and a
    /// half.
    #[test]
    fn a_stage_never_reached_is_absent_not_zero() {
        let _l = guard();
        for _ in 0..20 {
            let id = begin_pass(0, None);
            stamp_at(id, Stage::PartialTranscript, 100_000);
            close(id); // no reference, no fire
        }
        let id = begin_pass(0, None);
        stamp_at(id, Stage::PartialTranscript, 100_000);
        stamp_at(id, Stage::ReferenceDetected, 120_000);
        stamp_at(id, Stage::FireSent, 1_620_000);
        close(id);
        let r = report(4);
        let fire = r
            .metrics
            .iter()
            .find(|m| m.metric == "reference_detection_to_fire")
            .expect("metric");
        assert_eq!(fire.samples, 1, "only the pass that fired has this span");
        assert_eq!(fire.p50_ms, Some(1501.0));
    }

    #[test]
    fn the_first_stamp_of_a_stage_wins() {
        let _l = guard();
        let id = begin_pass(0, None);
        stamp_at(id, Stage::PartialTranscript, 50_000);
        stamp_at(id, Stage::PartialTranscript, 900_000);
        close(id);
        let r = report(4);
        assert_eq!(
            r.recent[0].at(Stage::PartialTranscript),
            Some(50_000),
            "a duplicate mark must not inflate the span"
        );
    }

    #[test]
    fn a_quantile_is_an_upper_bound_not_a_midpoint() {
        let _l = guard();
        let mut h = Hist::default();
        for _ in 0..95 {
            h.add(100_000); // 100ms
        }
        for _ in 0..5 {
            h.add(3_000_000); // 3s
        }
        // 95% were under 101ms — the exclusive top of the bucket holding 100ms.
        assert_eq!(h.quantile(0.95), Some(101_000));
        assert_eq!(h.quantile(0.99), Some(3_001_000));
        assert_eq!(h.max, 3_000_000);
    }

    /// A tail past the ceiling must be COUNTED, not dropped. A histogram that
    /// silently discards its worst samples reports a P95 that improves as the
    /// pipeline gets worse.
    #[test]
    fn samples_past_the_ceiling_are_counted_and_reported() {
        let _l = guard();
        let mut h = Hist::default();
        h.add(1_000);
        h.add(60_000_000); // a minute — far past the ceiling
        assert_eq!(h.count(), 2);
        assert_eq!(h.over, 1);
        assert_eq!(h.max, 60_000_000);
    }

    #[test]
    fn cadence_is_the_gap_between_consecutive_transcripts() {
        let _l = guard();
        for _ in 0..3 {
            let id = begin_pass(0, None);
            transcript_emitted(id, 1_000, 8_000, 1, false);
            close(id);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let r = report(4);
        let c = r
            .metrics
            .iter()
            .find(|m| m.metric == "transcript_cadence")
            .expect("metric");
        // Two gaps between three transcripts — never three.
        assert_eq!(c.samples, 2);
        assert!(r.transcript_updates_per_s.is_some());
    }

    /// A GAP AFTER SILENCE IS NOT A CADENCE. The speaker stopped; the worker has
    /// nothing to decode. Timing it reports the pauses in a sermon as pipeline
    /// latency, and one pause between readings is enough to make the worst case
    /// forty times the median.
    #[test]
    fn the_pause_after_a_closed_utterance_is_not_counted_as_cadence() {
        let _l = guard();
        // Two partials, then a final: one real gap between the partials.
        for is_final in [false, false, true] {
            let id = begin_pass(0, None);
            transcript_emitted(id, 1_000, 8_000, 1, is_final);
            close(id);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // A long silence, then speech resumes. It is an ORDER OF MAGNITUDE longer
        // than the gaps above so that a loaded CI runner overrunning a 10ms sleep
        // can never be mistaken for the pause having leaked in — the bound below
        // sits in the empty space between the two populations, not next to one.
        std::thread::sleep(std::time::Duration::from_millis(600));
        let id = begin_pass(0, None);
        transcript_emitted(id, 1_000, 8_000, 1, false);
        close(id);

        let r = report(8);
        let c = r
            .metrics
            .iter()
            .find(|m| m.metric == "transcript_cadence")
            .expect("metric");
        // partial→partial and partial→final. NOT final→partial.
        assert_eq!(c.samples, 2, "the silence was timed as if it were latency");
        assert!(
            c.worst_ms.unwrap_or(0.0) < 300.0,
            "worst {:?} — the pause leaked in",
            c.worst_ms
        );
    }

    /// The drift series is the whole point of keeping histograms for a service
    /// rather than a ring for a minute: "the transcript was fine for ten minutes
    /// and three seconds behind by the end" has to be visible, and a single P50
    /// over the whole service hides it completely.
    #[test]
    fn per_minute_means_expose_a_pipeline_that_degrades() {
        let _l = guard();
        {
            let r = recorder();
            let mut g = r.inner.lock().expect("lock");
            let i = Metric::AudioToPartial as usize;
            for minute in 0..5u64 {
                let at = minute * DRIFT_BUCKET_US;
                let us = 100_000 + minute * 200_000; // 100ms growing to 900ms
                g.drifts[i].add(at, us);
                g.hists[i].add(us);
            }
        }
        let r = report(4);
        let m = r
            .metrics
            .iter()
            .find(|m| m.metric == "audio_to_partial_transcript")
            .expect("metric");
        assert_eq!(
            m.per_minute_mean_ms,
            vec![100.0, 300.0, 500.0, 700.0, 900.0]
        );
    }

    #[test]
    fn an_epoch_stamp_lands_on_the_monotonic_timeline() {
        let _l = guard();
        let (epoch0, mono0) = epoch_anchor();
        let one_second_later_ms = (epoch0 / 1_000) + 1_000;
        let placed = from_epoch_ms(one_second_later_ms);
        // Within a millisecond of "the anchor plus a second" — the anchor's own
        // sub-millisecond remainder is the only error.
        assert!(
            placed.abs_diff(mono0 + 1_000_000) < 1_000,
            "placed {placed} vs {}",
            mono0 + 1_000_000
        );
    }

    /// A wall clock that steps BACKWARDS mid-service must not produce a stage that
    /// appears to have happened before the audio arrived. The arrival stamp is the
    /// backstop, and it is always on the right timeline.
    #[test]
    fn a_frontend_clock_from_the_future_falls_back_to_arrival() {
        let _l = guard();
        let id = begin_pass(0, None);
        transcript_emitted(id, 1_000, 8_000, 1, false);
        let (epoch0, _) = epoch_anchor();
        let year_from_now_ms = (epoch0 / 1_000) + 365 * 24 * 3_600 * 1_000;
        frontend_mark(id, Stage::TranscriptRendered, year_from_now_ms);
        close(id);
        let r = report(4);
        let t = &r.recent[0];
        let rendered = t.at(Stage::TranscriptRendered).expect("stamped");
        let emitted = t.at(Stage::PartialTranscript).expect("stamped");
        assert!(
            rendered >= emitted && rendered - emitted < 1_000_000,
            "a bogus clock must not invent a year of latency"
        );
    }

    #[test]
    fn open_traces_are_bounded_by_the_ring() {
        let _l = guard();
        for _ in 0..(RING * 3) {
            let id = begin_pass(0, None);
            transcript_emitted(id, 1_000, 8_000, 1, false);
            // deliberately never closed
        }
        let r = recorder();
        let g = r.inner.lock().expect("lock");
        assert!(g.open.len() <= RING, "open traces leaked: {}", g.open.len());
    }

    /// Evicting an open trace must not lose the spans it DID complete. Most passes
    /// never fire, so if eviction dropped them the report would be built from the
    /// small minority that did — and the transcript latency, the number this whole
    /// module exists for, would be measured over almost nothing.
    #[test]
    fn an_evicted_trace_still_contributes_the_spans_it_completed() {
        let _l = guard();
        for _ in 0..(RING + 10) {
            let id = begin_pass(0, None);
            stamp_at(id, Stage::PartialTranscript, 250_000);
            // never closed — eviction is the only thing that retires these
        }
        let r = report(4);
        let m = r
            .metrics
            .iter()
            .find(|m| m.metric == "audio_to_partial_transcript")
            .expect("metric");
        assert!(
            m.samples >= 10,
            "evicted traces were dropped: {}",
            m.samples
        );
    }

    /// THE STRUCTURE, PINNED STATICALLY. The decoder's callback must hand its
    /// transcript OFF and return; it must not decide what the words mean.
    ///
    /// This is a static check because the alternative is no check: driving the real
    /// whisper worker needs a model and a microphone, so a regression here — someone
    /// "simplifying" by calling `emit_detections` from the callback again — would
    /// pass every test in this repository while putting the semantic scan, three
    /// locks, a SQLite write and the kiosk fan-out back on the cadence.
    ///
    /// CLAUDE.md: a contract stated in a comment is not a contract.
    #[test]
    fn detection_does_not_run_on_the_decoders_thread() {
        let src = include_str!("main.rs");
        let at = src
            .find("SttEngine::try_load(path, move |update|")
            .expect("the STT callback moved — re-point this test, do not delete it");
        // The callback body, generously bounded. Long enough to catch anything
        // pasted back into it, short enough not to reach the next function.
        let body = &src[at..(at + 2_000).min(src.len())];
        assert!(
            !body.contains("emit_detections"),
            "detection is being called from the STT callback again — it runs              between one decode and the worker's next recv(), so every millisecond              of it is a millisecond the microphone is not being listened to"
        );
        assert!(
            body.contains("try_send"),
            "the callback no longer hands off over a BOUNDED queue — an unbounded              one does not prevent falling behind, it hides it"
        );
    }

    #[test]
    fn disabled_records_nothing_and_still_hands_out_ids() {
        let _l = guard();
        set_enabled(false);
        let a = begin_pass(0, None);
        let b = begin_pass(0, None);
        assert_ne!(a, b, "ids stay unique so callers never alias a trace");
        transcript_emitted(a, 1_000, 8_000, 1, false);
        close(a);
        let r = report(4);
        assert!(r.recent.is_empty());
        set_enabled(true);
    }
}
