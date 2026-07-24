//! Adaptive smooth stream — drip-reveal bursty network chunks (RongleCat/smoothStream port).

/// How many chars to reveal given current backlog size.
pub fn chars_to_reveal(backlog: usize) -> usize {
    if backlog == 0 {
        return 0;
    }
    if backlog <= 3 {
        return 1;
    }
    if backlog <= 12 {
        return 2;
    }
    if backlog <= 32 {
        return 4;
    }
    if backlog <= 80 {
        return 8;
    }
    if backlog <= 160 {
        return 16;
    }
    // ~45%/step — multi-KB bursts clear in ~15 frames
    (backlog as f32 * 0.45).ceil().max(24.0) as usize
}

pub fn next_displayed_len(current: usize, target: usize) -> usize {
    if current >= target {
        return target;
    }
    (current + chars_to_reveal(target - current)).min(target)
}

/// Advance displayed text one step toward target.
/// If target no longer starts with displayed (message swap), snap.
pub fn step_displayed(displayed: &str, target: &str) -> String {
    if target.is_empty() {
        return String::new();
    }
    if displayed.is_empty() {
        let n = chars_to_reveal(target.chars().count());
        return target.chars().take(n).collect();
    }
    if !target.starts_with(displayed) {
        return target.to_string();
    }
    if displayed.len() >= target.len() {
        return target.to_string();
    }
    let cur_chars = displayed.chars().count();
    let tgt_chars = target.chars().count();
    let next_n = next_displayed_len(cur_chars, tgt_chars);
    target.chars().take(next_n).collect()
}

/// Per-message smooth display buffer for the UI thread.
#[derive(Debug, Default, Clone)]
pub struct SmoothStream {
    pub target: String,
    pub displayed: String,
    pub active: bool,
}

impl SmoothStream {
    pub fn push_chunk(&mut self, chunk: &str) {
        self.target.push_str(chunk);
        self.active = true;
    }

    pub fn set_target(&mut self, text: String, active: bool) {
        if !text.starts_with(&self.displayed) {
            self.displayed.clear();
        }
        self.target = text;
        self.active = active;
        if !active {
            self.displayed = self.target.clone();
        }
    }

    pub fn finish(&mut self) {
        self.active = false;
        self.displayed = self.target.clone();
    }

    pub fn clear(&mut self) {
        self.target.clear();
        self.displayed.clear();
        self.active = false;
    }

    /// One frame step. Returns true if display changed.
    pub fn tick(&mut self) -> bool {
        if !self.active {
            if self.displayed != self.target {
                self.displayed = self.target.clone();
                return true;
            }
            return false;
        }
        let next = step_displayed(&self.displayed, &self.target);
        if next != self.displayed {
            self.displayed = next;
            true
        } else {
            false
        }
    }

    pub fn is_caught_up(&self) -> bool {
        self.displayed.len() >= self.target.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drip_then_catchup() {
        let mut d = String::new();
        let target = "hello world from grok";
        for _ in 0..50 {
            d = step_displayed(&d, target);
            if d == target {
                break;
            }
        }
        assert_eq!(d, target);
    }

    #[test]
    fn snap_on_replace() {
        let d = step_displayed("old prefix", "brand new");
        assert_eq!(d, "brand new");
    }
}
