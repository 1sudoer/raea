//! Where a video splits: at every cut the timeline knows (an outermost Sequence's start, a camera the engine cut to),
//! never inside a shot. The frames' side finds them as each frame is made; the encoder's side starts a segment at one,
//! unless the segment it's in is shorter than the minimum, so a shot under it goes on in the segment it started in.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

/// The cuts a render has found, shared between the thread that makes its frames and the encoder's.
#[derive(Clone, Debug, Default)]
pub struct Cuts(Arc<Mutex<BTreeSet<i64>>>);

impl Cuts {
    /// Frame `f`'s cuts: one when an outermost Sequence starts there (`starts`: each one's start frame, as
    /// raea_runtime_react::scene::shot_starts reads them) or the 3D's camera cut there (`camera`); frame 0 is no cut.
    pub fn find(&self, f: i64, starts: impl IntoIterator<Item = i64>, camera: bool) {
        if f > 0 && (camera || starts.into_iter().any(|s| s == f)) {
            self.mark(f);
        }
    }

    pub fn mark(&self, f: i64) {
        self.0.lock().unwrap().insert(f);
    }

    /// Whether frame `f` starts a shot.
    pub fn at(&self, f: i64) -> bool {
        self.0.lock().unwrap().contains(&f)
    }

    pub fn all(&self) -> Vec<i64> {
        self.0.lock().unwrap().iter().copied().collect()
    }

    /// Whether frame `f` starts a segment after one of `len` frames: at a cut, once that one has `min`.
    pub fn starts(&self, f: i64, len: usize, min: usize) -> bool {
        len >= min && self.at(f)
    }
}


/// The segments a run of `n` frames makes with these cuts: each one's first frame and length.
pub fn segments(n: usize, cuts: &Cuts, min: usize) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for i in 0..n {
        match out.last_mut() {
            Some((_, len)) if !cuts.starts(i as i64, *len, min) => *len += 1,
            _ => out.push((i, 1)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_starts_a_segment_unless_the_one_before_is_short() {
        let c = Cuts::default();
        c.find(0, [0], true);
        c.find(40, [0, 40], false);
        c.find(50, [], true);
        c.find(70, [10], false);
        assert_eq!(c.all(), vec![40, 50]);
        // 0..40 is long enough, 40..50 isn't: the cut at 50 is passed over
        assert_eq!(segments(90, &c, 30), vec![(0, 40), (40, 50)]);
        assert_eq!(segments(90, &c, 5), vec![(0, 40), (40, 10), (50, 40)]);
        assert_eq!(segments(30, &Cuts::default(), 30), vec![(0, 30)]);
    }
}
