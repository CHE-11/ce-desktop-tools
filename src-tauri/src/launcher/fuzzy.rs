//! Ranks a launcher entry against what was typed. Higher is better.

pub fn score(query: &str, text: &str) -> Option<i32> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let text = text.to_lowercase();
    let length_penalty = text.chars().count().min(200) as i32;
    if text == query {
        return Some(1200);
    }
    if text.starts_with(&query) {
        return Some(1000 - length_penalty);
    }
    if let Some(position) = word_start(&text, &query) {
        return Some(800 - position.min(200) as i32 - length_penalty / 4);
    }
    if let Some(position) = text.find(&query) {
        return Some(600 - position.min(200) as i32 - length_penalty / 4);
    }
    subsequence(&query, &text).map(|gaps| 300 - gaps.min(250) as i32 - length_penalty / 4)
}

fn word_start(text: &str, query: &str) -> Option<usize> {
    text.match_indices(query).map(|(index, _)| index).find(|&index| {
        index > 0
            && text[..index]
                .chars()
                .next_back()
                .is_some_and(|ch| !ch.is_alphanumeric())
    })
}

/// Every typed character appears in order. Returns the characters skipped
/// between the first and last match.
fn subsequence(query: &str, text: &str) -> Option<usize> {
    let mut wanted = query.chars().filter(|ch| !ch.is_whitespace()).peekable();
    let mut first = None;
    let mut last = 0;
    let mut matched = 0;
    for (index, ch) in text.chars().enumerate() {
        let Some(&next) = wanted.peek() else {
            break;
        };
        if ch == next {
            wanted.next();
            first.get_or_insert(index);
            last = index;
            matched += 1;
        }
    }
    if wanted.peek().is_some() {
        return None;
    }
    let first = first?;
    Some(last - first + 1 - matched)
}

#[cfg(test)]
mod tests {
    use super::score;

    #[test]
    fn prefix_beats_word_start_beats_substring_beats_subsequence() {
        let prefix = score("chr", "Chrome").unwrap();
        let word = score("code", "Visual Studio Code").unwrap();
        let inside = score("rome", "Chrome").unwrap();
        let loose = score("vsc", "Visual Studio Code").unwrap();
        assert!(prefix > word);
        assert!(word > inside);
        assert!(inside > loose);
        assert_eq!(score("note", "Notepad"), Some(1000 - 7));
    }

    #[test]
    fn missing_letters_do_not_match() {
        assert!(score("xyz", "Chrome").is_none());
        assert!(score("cmorh", "Chrome").is_none());
        assert_eq!(score("", "anything"), Some(0));
    }
}
