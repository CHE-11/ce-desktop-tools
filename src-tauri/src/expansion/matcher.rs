//! In-memory trigger matching. The typed buffer never leaves this process.

const MIN_CAP: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpansionHit {
    pub trigger: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Snippet {
    pub trigger: String,
    pub text: String,
    pub enabled: bool,
}

#[derive(Debug)]
pub struct Matcher {
    buffer: String,
    cap: usize,
}

impl Default for Matcher {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            cap: MIN_CAP,
        }
    }
}

impl Matcher {
    pub fn set_cap_from(&mut self, snippets: &[Snippet]) {
        self.cap = snippets
            .iter()
            .filter(|snippet| snippet.enabled)
            .map(|snippet| snippet.trigger.chars().count())
            .max()
            .unwrap_or(MIN_CAP)
            .max(MIN_CAP);
        self.trim();
    }

    pub fn push(&mut self, ch: char, snippets: &[Snippet]) -> Option<ExpansionHit> {
        self.buffer.push(ch);
        self.trim();
        best_match(&self.buffer, snippets)
    }

    pub fn backspace(&mut self) {
        self.buffer.pop();
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    fn trim(&mut self) {
        let count = self.buffer.chars().count();
        if count <= self.cap {
            return;
        }
        let drop_chars = count - self.cap;
        let mut end = self.buffer.len();
        for (index, (byte, _)) in self.buffer.char_indices().enumerate() {
            if index == drop_chars {
                end = byte;
                break;
            }
        }
        self.buffer.drain(..end);
    }
}

fn best_match(buffer: &str, snippets: &[Snippet]) -> Option<ExpansionHit> {
    let mut best: Option<&Snippet> = None;
    for snippet in snippets {
        if !snippet.enabled || snippet.trigger.is_empty() {
            continue;
        }
        if !is_token(buffer, &snippet.trigger) {
            continue;
        }
        let longer = best
            .map(|current| snippet.trigger.chars().count() > current.trigger.chars().count())
            .unwrap_or(true);
        if longer {
            best = Some(snippet);
        }
    }
    best.map(|snippet| ExpansionHit {
        trigger: snippet.trigger.clone(),
        text: snippet.text.clone(),
    })
}

/// The trigger is a suffix, and the character before it is the start of the
/// buffer or not a letter or digit.
fn is_token(buffer: &str, trigger: &str) -> bool {
    let Some(rest) = buffer.strip_suffix(trigger) else {
        return false;
    };
    match rest.chars().next_back() {
        None => true,
        Some(ch) => !ch.is_alphanumeric(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Matcher, Snippet};

    fn snippet(trigger: &str, text: &str, enabled: bool) -> Snippet {
        Snippet {
            trigger: trigger.to_string(),
            text: text.to_string(),
            enabled,
        }
    }

    #[test]
    fn longest_trigger_wins_when_both_are_tokens() {
        let snippets = vec![snippet("sig", "short", true), snippet(";sig", "long", true)];
        let mut matcher = Matcher::default();
        let mut hit = None;
        for ch in ";sig".chars() {
            hit = matcher.push(ch, &snippets);
        }
        assert_eq!(hit.unwrap().text, "long");
    }

    #[test]
    fn a_trigger_inside_a_word_does_not_match() {
        let snippets = vec![snippet("sig", "nope", true)];
        let mut matcher = Matcher::default();
        let mut hit = None;
        for ch in "mysig".chars() {
            hit = matcher.push(ch, &snippets);
        }
        assert!(hit.is_none());
    }

    #[test]
    fn punctuation_before_a_shorter_trigger_still_matches() {
        let snippets = vec![snippet("sig", "short", true), snippet(";sig", "long", true)];
        let mut matcher = Matcher::default();
        let mut hit = None;
        for ch in "x;sig".chars() {
            hit = matcher.push(ch, &snippets);
        }
        assert_eq!(hit.unwrap().text, "short");
    }

    #[test]
    fn disabled_snippets_are_ignored() {
        let snippets = vec![snippet(";sig", "nope", false)];
        let mut matcher = Matcher::default();
        let mut hit = None;
        for ch in ";sig".chars() {
            hit = matcher.push(ch, &snippets);
        }
        assert!(hit.is_none());
    }

    #[test]
    fn backspace_pops_the_buffer() {
        let snippets = vec![snippet("sig", "yes", true)];
        let mut matcher = Matcher::default();
        for ch in "sig".chars().take(2) {
            assert!(matcher.push(ch, &snippets).is_none());
        }
        matcher.backspace();
        assert!(matcher.push('g', &snippets).is_none());
        matcher.backspace();
        let hit = matcher.push('i', &snippets);
        assert!(hit.is_none());
        let hit = matcher.push('g', &snippets);
        assert_eq!(hit.unwrap().trigger, "sig");
    }
}
