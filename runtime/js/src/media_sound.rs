//! A playing `video`'s or `audio`'s sound (media_html.rs's stream plays it): its source's samples out of the default
//! output device through cpal, resampled to the device's rate, from a time on. Where the sound has got to is the
//! element's clock, so a video's frames follow its sound and the two can't drift apart over a long film. A muted element
//! plays silence on the same clock. RAEA_AUDIO_MUTE=1 (agents' runs, tests) opens no device: the stream keeps the wall's
//! clock instead.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// No time asked for (`Shared::jump`).
const NONE: u64 = u64::MAX;

struct Shared {
    /// where the sound is in the source, in source samples (f64 bits): the end of what the device's callback last
    /// wrote, or where `start` put it
    pos: AtomicU64,
    /// where that callback's samples began, in source samples (f64 bits): the time is read from here, on the wall's
    /// clock since the callback, and never past `pos`
    from: AtomicU64,
    /// when that callback ran (or `start`), in ns since `base()`
    when: AtomicU64,
    /// a time asked for, in source samples (f64 bits), taken by the next callback; NONE when there's none
    jump: AtomicU64,
    /// counts `start`s: a callback that ran across one doesn't write where it got to over the jump
    starts: AtomicU64,
    /// the clock's last reading, in seconds (f64 bits): it never goes back between starts
    read: AtomicU64,
    /// the device's delay from a callback to its sound reaching the speaker, in seconds (f64 bits)
    delay: AtomicU64,
    muted: AtomicBool,
    /// the element's volume, 0..1 (f32 bits): the gain while it isn't muted
    volume: AtomicU32,
    running: AtomicBool,
}

/// The moment the sound clocks count from (Shared::when).
fn base() -> std::time::Instant {
    static BASE: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *BASE.get_or_init(std::time::Instant::now)
}

/// Now, in ns since `base()`.
fn now_ns() -> u64 {
    base().elapsed().as_nanos() as u64
}

impl Shared {
    fn new(muted: bool) -> Shared {
        let zero = 0f64.to_bits();
        Shared { pos: AtomicU64::new(zero), from: AtomicU64::new(zero), when: AtomicU64::new(now_ns()), jump: AtomicU64::new(NONE), starts: AtomicU64::new(0), read: AtomicU64::new(zero), delay: AtomicU64::new(zero), muted: AtomicBool::new(muted), volume: AtomicU32::new(1f32.to_bits()), running: AtomicBool::new(false) }
    }

    /// A callback beginning: where its samples start (a jump asked for, else where the last one ended), and the
    /// `start`s so far, to hand back to `wrote`.
    fn begin(&self) -> (f64, u64) {
        let starts = self.starts.load(Relaxed);
        let jump = self.jump.swap(NONE, Relaxed);
        (if jump != NONE { f64::from_bits(jump) } else { f64::from_bits(self.pos.load(Relaxed)) }, starts)
    }

    /// A callback's samples written, `from` to `to` (source samples), at `when` (ns since `base()`). One that ran
    /// across a `start` leaves the jump it asked for in place: written over it, the clock read the old place until
    /// the next callback (a seek back read as the end of the film: the loop went round twice, frames came twice).
    fn wrote(&self, from: f64, to: f64, when: u64, starts: u64) {
        if self.starts.load(Relaxed) != starts {
            return;
        }
        self.from.store(from.to_bits(), Relaxed);
        self.when.store(when, Relaxed);
        self.pos.store(to.to_bits(), Relaxed);
    }

    /// Plays from `p` (source samples): the next callback starts there, and the clock reads from there at once.
    fn start(&self, p: f64) {
        self.starts.fetch_add(1, Relaxed);
        self.jump.store(p.to_bits(), Relaxed);
        self.from.store(p.to_bits(), Relaxed);
        self.pos.store(p.to_bits(), Relaxed);
        self.when.store(now_ns(), Relaxed);
        self.read.store(f64::NEG_INFINITY.to_bits(), Relaxed);
    }

    /// The source time reaching the speaker at `now` (ns since `base()`), in seconds, from `rate` samples a second:
    /// the last callback's start, moved on by the wall's time since it ran, never past what it wrote (a late callback
    /// holds the picture rather than run ahead of the sound) and never back from the last reading. Read as the end of
    /// what was last written, as it was, the clock stood still between callbacks and jumped a buffer at each (10 ms
    /// steps on this Mac), and a 60 fps film's frames came 6 to 27 ms apart where 16.7 is even.
    fn time_at(&self, now: u64, rate: f64) -> f64 {
        let (from, to) = (f64::from_bits(self.from.load(Relaxed)), f64::from_bits(self.pos.load(Relaxed)));
        let since = now.saturating_sub(self.when.load(Relaxed)) as f64 / 1e9;
        let t = (from / rate + since).min(to / rate) - f64::from_bits(self.delay.load(Relaxed));
        let last = f64::from_bits(self.read.fetch_max_f64(t));
        t.max(last)
    }
}

/// `fetch_max` for an f64 kept as its bits.
trait FetchMaxF64 {
    /// Stores the larger of what's there and `v`; returns what was there.
    fn fetch_max_f64(&self, v: f64) -> u64;
}

impl FetchMaxF64 for AtomicU64 {
    fn fetch_max_f64(&self, v: f64) -> u64 {
        let mut cur = self.load(Relaxed);
        loop {
            if f64::from_bits(cur) >= v {
                return cur;
            }
            match self.compare_exchange_weak(cur, v.to_bits(), Relaxed, Relaxed) {
                Ok(_) => return cur,
                Err(now) => cur = now,
            }
        }
    }
}

/// The device's stream for one element's sound.
pub struct Sound {
    stream: cpal::Stream,
    shared: Arc<Shared>,
    rate: f64,
    len: f64,
}

impl Sound {
    /// The default output device, playing `audio` when started; an error when there's no device (or it's muted for
    /// the whole process, or the source has no sound).
    pub fn open(audio: raea_io_media::Audio, muted: bool) -> Result<Sound, String> {
        if std::env::var("RAEA_AUDIO_MUTE").as_deref() == Ok("1") {
            return Err("RAEA_AUDIO_MUTE=1".into());
        }
        let len = audio.len();
        if len == 0 || audio.rate == 0 {
            return Err("no sound".into());
        }
        let dev = cpal::default_host().default_output_device().ok_or("no audio output device")?;
        let def = dev.default_output_config().map_err(|e| format!("audio output: {e}"))?;
        if def.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!("audio output: {:?} samples, not f32", def.sample_format()));
        }
        let ch = (def.channels() as usize).max(1);
        let rate = audio.rate as f64;
        let step = rate / def.sample_rate().0 as f64;
        let shared = Arc::new(Shared::new(muted));
        let s = shared.clone();
        let src = audio.channels;
        let last = len - 1;
        // the sample at fractional position `p` of channel `c`, straight-line between its neighbours
        let at = move |c: &[f32], p: f64| -> f32 {
            let i = p as usize;
            if i >= last {
                return if i == last { c[last] } else { 0.0 };
            }
            let f = (p - i as f64) as f32;
            c[i] + (c[i + 1] - c[i]) * f
        };
        let callback = move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
            let when = now_ns();
            let ts = info.timestamp();
            if let Some(d) = ts.playback.duration_since(&ts.callback) {
                s.delay.store(d.as_secs_f64().to_bits(), Relaxed);
            }
            let (from, starts) = s.begin();
            let mut p = from;
            if !s.running.load(Relaxed) {
                out.fill(0.0);
                s.wrote(p, p, when, starts);
                return;
            }
            let gain = if s.muted.load(Relaxed) { 0.0 } else { f32::from_bits(s.volume.load(Relaxed)) };
            for frame in out.chunks_mut(ch) {
                if p as usize > last {
                    frame.fill(0.0);
                } else if src.len() == 1 || ch == 1 {
                    // mono out of a stereo source, or a mono source on every speaker
                    let v = src.iter().map(|c| at(c, p)).sum::<f32>() / src.len() as f32 * gain;
                    frame.fill(v);
                } else {
                    for (k, o) in frame.iter_mut().enumerate() {
                        *o = src.get(k).map(|c| at(c, p) * gain).unwrap_or(0.0);
                    }
                }
                p += step;
            }
            s.wrote(from, p, when, starts);
        };
        let stream = dev.build_output_stream(&def.config(), callback, |e| eprintln!("raea-runtime-js: media sound: {e}"), None).map_err(|e| format!("audio stream: {e}"))?;
        Ok(Sound { stream, shared, rate, len: len as f64 / rate })
    }

    /// Plays from `t` seconds on.
    pub fn start(&self, t: f64) {
        self.shared.start(t.max(0.0) * self.rate);
        self.shared.running.store(true, Relaxed);
        let _ = self.stream.play();
    }

    pub fn stop(&self) {
        self.shared.running.store(false, Relaxed);
        let _ = self.stream.pause();
    }

    pub fn set_muted(&self, muted: bool) {
        self.shared.muted.store(muted, Relaxed);
    }

    /// How loud it plays while it isn't muted, 0..1 (a page's element.volume).
    pub fn set_volume(&self, volume: f32) {
        let v = if volume.is_finite() { volume.clamp(0.0, 1.0) } else { 1.0 };
        self.shared.volume.store(v.to_bits(), Relaxed);
    }

    /// The source time now reaching the speaker: what the picture shows to be in step with it (`Shared::time_at`).
    pub fn time(&self) -> f64 {
        self.shared.time_at(now_ns(), self.rate).clamp(0.0, self.len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;
    /// a device's buffer: 512 samples, 10.7 ms at 48 kHz
    const BUF: f64 = 512.0;
    const MS: u64 = 1_000_000;

    /// A callback at `at` (ns since `base()`) writing one buffer on from where the last ended.
    fn callback(s: &Shared, at: u64) {
        let (from, starts) = s.begin();
        s.wrote(from, from + BUF, at, starts);
    }

    /// Between two callbacks the clock moves on with the wall's, a millisecond a millisecond: read as the end of
    /// what was last written, it stood still for a buffer and then jumped one (11-16).
    #[test]
    fn the_clock_moves_between_callbacks() {
        let s = Shared::new(false);
        s.start(0.0);
        let t0 = now_ns();
        let (mut last, mut most) = (f64::NAN, 0f64);
        for ms in 0..60u64 {
            // a callback each buffer, right on time
            let at = t0 + ms * MS;
            if ms % 11 == 0 {
                callback(&s, at);
            }
            let t = s.time_at(at, RATE);
            if !last.is_nan() {
                most = most.max(t - last);
                assert!(t >= last, "the clock went back: {t} after {last}");
            }
            last = t;
        }
        assert!(most < 0.0015, "the clock stepped {most} s in a millisecond");
        // and it keeps the wall's pace: 59 ms on, about 59 ms of sound
        assert!((last - 0.059).abs() < 0.003, "{last}");
    }

    /// A callback that comes late holds the clock at the end of what was written rather than running ahead of the
    /// sound; the next one takes it on from there.
    #[test]
    fn a_late_callback_holds_the_clock() {
        let s = Shared::new(false);
        s.start(0.0);
        let t0 = now_ns();
        callback(&s, t0);
        let end = BUF / RATE;
        assert!((s.time_at(t0 + 30 * MS, RATE) - end).abs() < 1e-9, "held at the buffer's end");
        callback(&s, t0 + 30 * MS);
        assert!((s.time_at(t0 + 31 * MS, RATE) - (end + 0.001)).abs() < 1e-6);
    }

    /// A start (a seek, a loop going round) made while a callback runs isn't written over by that callback: the clock
    /// reads the new place at once and after it ends, and the next callback plays from there.
    #[test]
    fn a_start_during_a_callback_stands() {
        let s = Shared::new(false);
        s.start(10.0 * RATE);
        let t0 = now_ns();
        callback(&s, t0);
        // a callback begins, the film goes back to 2 s, the callback ends
        let (from, starts) = s.begin();
        s.start(2.0 * RATE);
        s.wrote(from, from + BUF, t0 + 11 * MS, starts);
        let t = s.time_at(now_ns(), RATE);
        assert!((2.0..2.1).contains(&t), "read {t} after the start at 2 s");
        // the next callback takes the jump
        let (p, _) = s.begin();
        assert_eq!(p, 2.0 * RATE);
    }

    /// The delay to the speaker is taken off, and a delay that grows doesn't take the clock back.
    #[test]
    fn the_delay_never_takes_it_back() {
        let s = Shared::new(false);
        s.start(RATE);
        let t0 = now_ns();
        callback(&s, t0);
        let before = s.time_at(t0 + 5 * MS, RATE);
        assert!((before - 1.005).abs() < 1e-6);
        s.delay.store(0.02f64.to_bits(), Relaxed);
        assert_eq!(s.time_at(t0 + 6 * MS, RATE), before, "held while the delay is made up");
        // (at 10 ms on, 1.010 - 0.02 is still behind what was read)
        assert_eq!(s.time_at(t0 + 10 * MS, RATE), before);
        // once a callback brings the sound past it, the clock goes on from there
        callback(&s, t0 + 11 * MS);
        callback(&s, t0 + 22 * MS);
        let t = s.time_at(t0 + 30 * MS, RATE);
        assert!((t - (1.0 + 2.0 * BUF / RATE + 0.008 - 0.02)).abs() < 1e-6, "{t}");
    }
}
