//! In-memory trigger matching. The typed buffer never leaves this process.

const MIN_CAP: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpansionHit {
    pub trigger: String,
    pub text: String,
    pub ending: Option<char>,
    pub timed: bool,
    pub secret: bool,
}

#[derive(Clone, Debug)]
pub struct Snippet {
    pub trigger: String,
    pub text: String,
    pub enabled: bool,
    pub ending: bool,
    pub timed: bool,
    pub ignore_case: bool,
    pub symbol: bool,
    pub secret: bool,
}

#[derive(Clone, Debug)]
struct Held {
    trigger: String,
    text: String,
    timed: bool,
    ignore_case: bool,
    secret: bool,
}

pub struct Matcher {
    buffer: String,
    cap: usize,
    held: Option<Held>,
}

impl Default for Matcher {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            cap: MIN_CAP,
            held: None,
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
        if let Some(hit) = best_match(&self.buffer, snippets) {
            if hit.ending.is_none() && has_longer_prefix(snippets, &hit.trigger) {
                let ignore_case = snippets.iter().any(|snippet| {
                    snippet.enabled
                        && !snippet.ending
                        && same_trigger(&snippet.trigger, &hit.trigger, snippet.ignore_case)
                });
                self.held = Some(Held {
                    trigger: hit.trigger,
                    text: hit.text,
                    timed: hit.timed,
                    ignore_case,
                    secret: hit.secret,
                });
                return None;
            }
            self.held = None;
            return Some(hit);
        }
        let Some(held) = self.held.take() else {
            return None;
        };
        if has_longer_prefix(snippets, &format!("{}{ch}", held.trigger)) {
            return None;
        }
        let without = &self.buffer[..self.buffer.len() - ch.len_utf8()];
        if is_token(without, &held.trigger, held.ignore_case) {
            return Some(ExpansionHit {
                trigger: held.trigger,
                text: held.text,
                ending: Some(ch),
                timed: held.timed,
                secret: held.secret,
            });
        }
        None
    }

    pub fn backspace(&mut self) {
        self.buffer.pop();
        self.held = None;
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.held = None;
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
    let mut best: Option<ExpansionHit> = None;
    for snippet in snippets {
        if !snippet.enabled || snippet.trigger.is_empty() {
            continue;
        }
        let ending = if snippet.ending {
            let Some(end) = ending_of(buffer, &snippet.trigger, snippet.ignore_case) else {
                continue;
            };
            Some(end)
        } else if is_token(buffer, &snippet.trigger, snippet.ignore_case) {
            None
        } else {
            continue;
        };
        let hit = ExpansionHit {
            trigger: snippet.trigger.clone(),
            text: snippet.text.clone(),
            ending,
            timed: snippet.timed,
            secret: snippet.secret,
        };
        if best
            .as_ref()
            .map(|current| prefers(&hit, current))
            .unwrap_or(true)
        {
            best = Some(hit);
        }
    }
    best
}

fn has_longer_prefix(snippets: &[Snippet], prefix: &str) -> bool {
    snippets.iter().any(|snippet| {
        snippet.enabled
            && !snippet.ending
            && snippet.trigger.chars().count() > prefix.chars().count()
            && trigger_has_prefix(&snippet.trigger, prefix, snippet.ignore_case)
    })
}

fn same_trigger(trigger: &str, other: &str, ignore_case: bool) -> bool {
    if ignore_case {
        trigger.eq_ignore_ascii_case(other)
    } else {
        trigger == other
    }
}

fn trigger_has_prefix(trigger: &str, prefix: &str, ignore_case: bool) -> bool {
    let trigger: Vec<char> = trigger.chars().collect();
    let prefix: Vec<char> = prefix.chars().collect();
    if prefix.len() > trigger.len() {
        return false;
    }
    if ignore_case {
        trigger[..prefix.len()]
            .iter()
            .zip(&prefix)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    } else {
        trigger[..prefix.len()] == prefix[..]
    }
}

fn prefers(candidate: &ExpansionHit, current: &ExpansionHit) -> bool {
    let left = candidate.trigger.chars().count();
    let right = current.trigger.chars().count();
    if left != right {
        return left > right;
    }
    !candidate.text.is_empty() && current.text.is_empty()
}

fn ending_of(buffer: &str, trigger: &str, ignore_case: bool) -> Option<char> {
    let end = buffer.chars().next_back()?;
    if !is_ending(end) {
        return None;
    }
    let word = &buffer[..buffer.len() - end.len_utf8()];
    if is_token(word, trigger, ignore_case) {
        Some(end)
    } else {
        None
    }
}

pub fn is_ending(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '\t'
            | '\n'
            | '\r'
            | '-'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | ':'
            | ';'
            | '\''
            | '"'
            | '`'
            | '/'
            | '\\'
            | ','
            | '.'
            | '?'
            | '!'
    )
}

/// The trigger is a suffix, and the character before it is the start of the
/// buffer or not a letter or digit.
fn is_token(buffer: &str, trigger: &str, ignore_case: bool) -> bool {
    let buffer_chars: Vec<char> = buffer.chars().collect();
    let trigger_chars: Vec<char> = trigger.chars().collect();
    if trigger_chars.is_empty() || buffer_chars.len() < trigger_chars.len() {
        return false;
    }
    let split = buffer_chars.len() - trigger_chars.len();
    let suffix_matches = if ignore_case {
        buffer_chars[split..]
            .iter()
            .zip(&trigger_chars)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    } else {
        buffer_chars[split..] == trigger_chars[..]
    };
    if !suffix_matches {
        return false;
    }
    match buffer_chars[..split].last() {
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
            ending: false,
            timed: false,
            ignore_case: false,
            symbol: false,
            secret: false,
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
    fn a_misspell_waits_for_a_space_and_keeps_its_case() {
        let snippets = vec![
            Snippet {
                trigger: "dont".into(),
                text: "don't".into(),
                enabled: true,
                ending: true,
                timed: false,
                ignore_case: false,
                symbol: false,
                secret: false,
            },
            Snippet {
                trigger: "Dont".into(),
                text: "Don't".into(),
                enabled: true,
                ending: true,
                timed: false,
                ignore_case: false,
                symbol: false,
                secret: false,
            },
        ];
        let mut matcher = Matcher::default();
        let mut hit = None;
        for ch in "dont".chars() {
            hit = matcher.push(ch, &snippets);
        }
        assert!(hit.is_none());
        hit = matcher.push(' ', &snippets);
        let hit = hit.unwrap();
        assert_eq!(hit.text, "don't");
        assert_eq!(hit.ending, Some(' '));

        let mut matcher = Matcher::default();
        let mut hit = None;
        for ch in "Dont ".chars() {
            hit = matcher.push(ch, &snippets);
        }
        assert_eq!(hit.unwrap().text, "Don't");
    }

    #[test]
    fn starter_misspells_replace_dont_only_after_a_space() {
        let snippets: Vec<Snippet> = crate::misspell::starters()
            .into_iter()
            .map(|entry| Snippet {
                trigger: entry.trigger,
                text: entry.text,
                enabled: entry.enabled,
                ending: true,
                timed: false,
                ignore_case: false,
                symbol: false,
                secret: false,
            })
            .collect();
        let mut matcher = Matcher::default();
        matcher.set_cap_from(&snippets);
        let mut hit = None;
        for ch in "dont".chars() {
            hit = matcher.push(ch, &snippets);
        }
        assert!(hit.is_none());
        let hit = matcher.push(' ', &snippets).unwrap();
        assert_eq!(hit.trigger, "dont");
        assert_eq!(hit.text, "don't");
        assert_eq!(hit.ending, Some(' '));
        let mut typed = hit.text.clone();
        typed.push(hit.ending.unwrap());
        assert_eq!(typed, "don't ");
        assert_eq!(hit.trigger.chars().count(), 4);

        let mut matcher = Matcher::default();
        matcher.set_cap_from(&snippets);
        let hit = "Dont "
            .chars()
            .fold(None, |_, ch| matcher.push(ch, &snippets))
            .unwrap();
        assert_eq!(hit.text, "Don't");
        assert_eq!(hit.ending, Some(' '));
    }

    #[test]
    fn a_misspell_inside_a_longer_word_does_not_match() {
        let snippets = vec![Snippet {
            trigger: "dont".into(),
            text: "don't".into(),
            enabled: true,
            ending: true,
            timed: false,
            ignore_case: false,
            symbol: false,
            secret: false,
        }];
        let mut matcher = Matcher::default();
        let mut hit = None;
        for ch in "dontworry ".chars() {
            hit = matcher.push(ch, &snippets);
        }
        assert!(hit.is_none());
    }

    #[test]
    fn a_short_symbol_waits_while_a_longer_trigger_can_still_match() {
        let snippets = vec![
            Snippet {
                trigger: "/".into(),
                text: "×".into(),
                enabled: true,
                ending: false,
                timed: false,
                ignore_case: true,
                symbol: true,
                secret: false,
            },
            Snippet {
                trigger: "/date".into(),
                text: "11/14/26".into(),
                enabled: true,
                ending: false,
                timed: true,
                ignore_case: true,
                symbol: false,
                secret: false,
            },
        ];
        let mut matcher = Matcher::default();
        assert!(matcher.push('/', &snippets).is_none());
        assert!(matcher.push('d', &snippets).is_none());
        let hit = "ate"
            .chars()
            .fold(None, |_, ch| matcher.push(ch, &snippets));
        assert_eq!(hit.unwrap().text, "11/14/26");

        let mut matcher = Matcher::default();
        assert!(matcher.push('/', &snippets).is_none());
        let hit = matcher.push(' ', &snippets).unwrap();
        assert_eq!(hit.text, "×");
        assert_eq!(hit.ending, Some(' '));
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
