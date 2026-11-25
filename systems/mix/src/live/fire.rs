//! Sounds fired now, off any timeline (docs/hardware.feature HW0a): a page's interaction sounds, raea/tone's `fire()`.
//! A stage's mixer places a timeline and plays each frame's events on it (`Live::sync`); a page has no frames, so its
//! events play at the clock's now plus the head plus their own time. One mixer does one or the other: the page's
//! (raea-page-sound) only fires, the stage's only syncs.

use super::*;

impl Live {
    /// raea/tone's `evs` played now, each at the clock's now plus the head plus its own time ("t", seconds). `graph`
    /// (raea/tone's graph JSON, as `<audio data-tone-graph>` holds it) is built when it is new or changed; without one
    /// the graph built last plays them. The call returns at once: the voices run on the device's clock.
    pub fn fire(&mut self, graph: Option<&str>, evs: &[serde_json::Value]) {
        self.wake();
        self.settle_clock();
        let now = self.cx.audio_clock_corrected().samples.0 + self.head_base;
        if let Some(g) = graph
            && raea_sys_tone::tonegraph::hash(g) != self.built.hash
        {
            self.built.tear_down(&mut self.cx, self.voices);
            match raea_sys_tone::tonegraph::build(&mut self.cx, self.voices, self.master, g, now, &mut |f| crate::open_whole(f)) {
                Ok(b) => self.built = b,
                Err(e) => eprintln!("raea mixer: {e}"),
            }
        }
        let rate = self.rate;
        let score = self.score.get_or_insert_with(|| Score::new(60.0, rate));
        // a fire is heard once: its keys are never asked for again, so none is kept
        score.forget_notes();
        for (t, out) in score.tone(0, &self.built, evs) {
            let at = Some(EventInstant::AtClockSamples(InstantSamples(now + t.max(0))));
            match out {
                ToneOut::Voice(ev) => self.cx.schedule_event_for(self.voices, event(ev), at),
                ToneOut::Fx(node, fe) => {
                    if let Some(id) = self.built.fx.get(&node) {
                        self.cx.schedule_event_for(*id, event(fe), at);
                    }
                }
            }
        }
        let _ = self.cx.update();
    }

    /// The graph's housekeeping between fires (what the audio thread hands back, the clock's correction): a firing
    /// mixer has no frames to do it on.
    pub fn tend(&mut self) {
        let _ = self.cx.update();
    }

    /// Late events so far: those that reached the audio thread after their sample (HW0b's count).
    pub fn late_events(&self) -> u64 {
        self.stats.late.load(Ordering::Relaxed) as u64
    }
}
