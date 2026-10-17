use rand::Rng;
use std::time::Duration;

/// Individual primitive in a synthetic typing sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// Key depressed down.
    KeyDown { char_code: u16, is_unicode: bool },
    /// Key released up.
    KeyUp { char_code: u16, is_unicode: bool },
    /// Delay before executing next action.
    Delay(Duration),
}

/// A complete, timed synthetic typing plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeystrokePlan {
    pub actions: Vec<KeyAction>,
    pub total_duration: Duration,
}

impl KeystrokePlan {
    /// Returns the total number of characters scheduled to be typed.
    pub fn key_count(&self) -> usize {
        self.actions
            .iter()
            .filter(|a| matches!(a, KeyAction::KeyDown { .. }))
            .count()
    }
}

/// Plans a sequence of keystrokes with human-mimicking timing jitter and key hold times.
pub fn plan_keystrokes(
    text: &str,
    press_enter: bool,
    jitter_range_ms: (u32, u32),
) -> KeystrokePlan {
    let mut rng = rand::thread_rng();
    plan_keystrokes_with_rng(text, press_enter, jitter_range_ms, &mut rng)
}

/// Plans keystrokes using a supplied RNG for deterministic verification.
pub fn plan_keystrokes_with_rng<R: Rng>(
    text: &str,
    press_enter: bool,
    jitter_range_ms: (u32, u32),
    rng: &mut R,
) -> KeystrokePlan {
    let (min_jitter, max_jitter) = if jitter_range_ms.0 <= jitter_range_ms.1 {
        (jitter_range_ms.0.max(10), jitter_range_ms.1.max(20))
    } else {
        (jitter_range_ms.1.max(10), jitter_range_ms.0.max(20))
    };

    let mut actions = Vec::new();
    let mut total_duration = Duration::ZERO;

    for ch in text.chars() {
        let code = ch as u16;

        // Key down
        actions.push(KeyAction::KeyDown {
            char_code: code,
            is_unicode: true,
        });

        // Key hold duration (human physical contact typically lasts 40-80ms)
        let hold_ms = rng.gen_range(40..=80);
        let hold_delay = Duration::from_millis(hold_ms);
        actions.push(KeyAction::Delay(hold_delay));
        total_duration += hold_delay;

        // Key up
        actions.push(KeyAction::KeyUp {
            char_code: code,
            is_unicode: true,
        });

        // Inter-keystroke jitter
        let mut inter_key_ms = rng.gen_range(min_jitter..=max_jitter);

        // Natural pause after spaces or punctuation
        if ch == ' ' || ch == '.' || ch == ',' || ch == '\n' {
            inter_key_ms += rng.gen_range(50..=120);
        }

        let inter_delay = Duration::from_millis(inter_key_ms as u64);
        actions.push(KeyAction::Delay(inter_delay));
        total_duration += inter_delay;
    }

    if press_enter {
        // Virtual key for Enter is 0x0D (VK_RETURN)
        let enter_code = 0x0D;
        actions.push(KeyAction::KeyDown {
            char_code: enter_code,
            is_unicode: false,
        });

        let enter_hold = Duration::from_millis(rng.gen_range(40..=75));
        actions.push(KeyAction::Delay(enter_hold));
        total_duration += enter_hold;

        actions.push(KeyAction::KeyUp {
            char_code: enter_code,
            is_unicode: false,
        });

        let post_enter_delay = Duration::from_millis(rng.gen_range(60..=120));
        actions.push(KeyAction::Delay(post_enter_delay));
        total_duration += post_enter_delay;
    }

    KeystrokePlan {
        actions,
        total_duration,
    }
}
