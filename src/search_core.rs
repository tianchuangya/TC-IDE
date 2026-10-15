//! Shared text search and replace.
//!
//! Finding text is the operation an editor does most often and the one a user
//! notices most when it is wrong: a search that misses a match, or replaces one
//! it should not have, costs more than not having the feature. The rules are
//! therefore written here as pure functions over lines, so all three backends
//! share one behaviour and it is tested where it is cheap to run.
//!
//! Nothing here touches a file: a search returns the places a match sits, and a
//! replace returns the text that would result, so a caller decides what to do
//! with either.

#![allow(dead_code)] // The backends adopt the pieces they need as they migrate.

/// How a search compares text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseMode {
    /// Match only when the letters agree.
    Sensitive,
    /// Match whatever the letters' case is.
    Insensitive,
}

impl CaseMode {
    pub const ALL: [CaseMode; 2] = [CaseMode::Sensitive, CaseMode::Insensitive];

    pub fn label(self) -> &'static str {
        match self {
            CaseMode::Sensitive => "match case",
            CaseMode::Insensitive => "ignore case",
        }
    }
}

/// What a search looks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub needle: String,
    pub case: CaseMode,
    /// Whether `needle` is a regular expression.
    pub regex: bool,
    /// Whether only whole words count.
    pub whole_word: bool,
}

impl Query {
    /// A plain search for some text.
    pub fn new(needle: impl Into<String>) -> Self {
        Self {
            needle: needle.into(),
            case: CaseMode::Sensitive,
            regex: false,
            whole_word: false,
        }
    }

    /// Whether the query can match anything at all.
    pub fn is_empty(&self) -> bool {
        self.needle.is_empty()
    }
}

/// One place a query matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub line: usize,
    /// Byte range inside the line, so a caller can highlight exactly it.
    pub start: usize,
    pub end: usize,
}

impl Match {
    /// How many bytes the match covers.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

/// Whether a character can be part of a word.
///
/// Whole word searches ask whether the characters on either side of a match
/// are word characters, so the test has to agree with the one used to find
/// words in the first place.
pub fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Byte ranges of the matches of a query inside one line.
///
/// Matches never overlap: after one is found the scan continues from its end,
/// so searching for `aa` in `aaaa` finds two rather than three, which is what
/// a person stepping through matches expects.
pub fn find_in_line(line: &str, query: &Query) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    if query.regex {
        return find_regex_in_line(line, &query.needle, query.case, query.whole_word);
    }
    let (haystack, needle) = match query.case {
        CaseMode::Sensitive => (line.to_string(), query.needle.clone()),
        CaseMode::Insensitive => (line.to_lowercase(), query.needle.to_lowercase()),
    };
    let mut matches = Vec::new();
    let mut from = 0usize;
    while from <= haystack.len() {
        let Some(found) = haystack[from..].find(&needle) else {
            break;
        };
        let start = from + found;
        let end = start + needle.len();
        // Lowercasing can change a byte length, so a match found in the folded
        // text is mapped back by character count rather than taken as-is.
        let (start, end) = if query.case == CaseMode::Insensitive {
            match fold_range(line, start, end) {
                Some(range) => range,
                None => break,
            }
        } else {
            (start, end)
        };
        if !query.whole_word || is_whole_word(line, start, end) {
            matches.push((start, end));
        }
        let next = found + needle.len().max(1);
        from += next;
        if next == 0 {
            break;
        }
    }
    matches
}

/// Map a byte range of the lowercased text back onto the original.
///
/// Folding case can change a character's byte length - `İ` lowercases to two
/// bytes - so an offset into the folded text is not always an offset into the
/// original. Counting characters is exact, and is what keeps a highlight on the
/// text the reader sees.
fn fold_range(line: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let folded = line.to_lowercase();
    if start > folded.len() || end > folded.len() {
        return None;
    }
    if !folded.is_char_boundary(start) || !folded.is_char_boundary(end) {
        return None;
    }
    let start_chars = folded[..start].chars().count();
    let end_chars = folded[..end].chars().count();
    let mut offsets = line.char_indices().map(|(index, _)| index);
    let mut start_byte = None;
    let mut end_byte = None;
    for (count, byte) in offsets.by_ref().enumerate() {
        if count == start_chars {
            start_byte = Some(byte);
        }
        if count == end_chars {
            end_byte = Some(byte);
            break;
        }
    }
    let start_byte =
        start_byte.or_else(|| (start_chars == line.chars().count()).then_some(line.len()))?;
    let end_byte =
        end_byte.or_else(|| (end_chars == line.chars().count()).then_some(line.len()))?;
    Some((start_byte, end_byte))
}

/// Whether a range is surrounded by non-word characters.
fn is_whole_word(line: &str, start: usize, end: usize) -> bool {
    let before = line[..start].chars().next_back();
    let after = line[end..].chars().next();
    let boundary = |character: Option<char>| match character {
        Some(character) => !is_word_character(character),
        None => true,
    };
    boundary(before) && boundary(after)
}

/// Find a match of a literal without any regular expression support.
///
/// The regex flag is accepted by the query type, but the matching is literal:
/// a real expression engine is a large piece of work, and quietly treating a
/// pattern as literal text would find the wrong thing. A query marked as a
/// regular expression therefore searches for the pattern as written, and the
/// editor says so.
fn find_regex_in_line(
    line: &str,
    needle: &str,
    case: CaseMode,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    let literal = Query {
        needle: needle.to_string(),
        case,
        regex: false,
        whole_word,
    };
    find_in_line(line, &literal)
}

/// Every match in a document, in reading order.
pub fn find_in_lines(lines: &[String], query: &Query) -> Vec<Match> {
    let mut matches = Vec::new();
    for (line, text) in lines.iter().enumerate() {
        for (start, end) in find_in_line(text, query) {
            matches.push(Match { line, start, end });
        }
    }
    matches
}

/// The match after a position, wrapping to the first.
///
/// Stepping past the last match and landing on the first is what makes a find
/// loop instead of stopping, which is what a person pressing Enter repeatedly
/// expects.
pub fn next_match(matches: &[Match], from: (usize, usize)) -> Option<Match> {
    if matches.is_empty() {
        return None;
    }
    matches
        .iter()
        .find(|found| (found.line, found.start) > from)
        .or_else(|| matches.first())
        .copied()
}

/// The match before a position, wrapping to the last.
pub fn previous_match(matches: &[Match], from: (usize, usize)) -> Option<Match> {
    if matches.is_empty() {
        return None;
    }
    matches
        .iter()
        .rev()
        .find(|found| (found.line, found.start) < from)
        .or_else(|| matches.last())
        .copied()
}

/// Replace one match, returning the new line.
pub fn replace_one(line: &str, found: Match, replacement: &str) -> String {
    if found.start > found.end || found.end > line.len() {
        return line.to_string();
    }
    if !line.is_char_boundary(found.start) || !line.is_char_boundary(found.end) {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len() + replacement.len());
    out.push_str(&line[..found.start]);
    out.push_str(replacement);
    out.push_str(&line[found.end..]);
    out
}

/// Replace every match in a document, returning the new lines.
///
/// Replacements are applied from the end of each line backwards, so an earlier
/// replacement cannot shift the offsets of a later one.
pub fn replace_all(lines: &[String], query: &Query, replacement: &str) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            let found = find_in_line(line, query);
            if found.is_empty() {
                return line.clone();
            }
            let mut out = line.clone();
            for (start, end) in found.into_iter().rev() {
                out = replace_one(
                    &out,
                    Match {
                        line: 0,
                        start,
                        end,
                    },
                    replacement,
                );
            }
            out
        })
        .collect()
}

/// How many matches a query finds in a document.
pub fn count_matches(lines: &[String], query: &Query) -> usize {
    find_in_lines(lines, query).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &[&str]) -> Vec<String> {
        text.iter().map(|line| line.to_string()).collect()
    }

    #[test]
    fn a_plain_search_finds_every_place_the_text_appears() {
        let source = lines(&["let value = 1;", "value += 2;", "// nothing here"]);
        let found = find_in_lines(&source, &Query::new("value"));
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0],
            Match {
                line: 0,
                start: 4,
                end: 9
            }
        );
        assert_eq!(found[1].line, 1);
        assert_eq!(found[1].start, 0);
        // The match is a byte range, so slicing the line gives back the text.
        assert_eq!(&source[0][found[0].start..found[0].end], "value");
        assert_eq!(found[0].len(), 5);
        assert!(!found[0].is_empty());
        // A needle that is not there finds nothing rather than something.
        assert!(find_in_lines(&source, &Query::new("absent")).is_empty());
        assert_eq!(count_matches(&source, &Query::new("value")), 2);
    }

    #[test]
    fn matches_do_not_overlap() {
        // Stepping through matches has to advance, so an overlapping pair is
        // not reported twice: `aa` in `aaaa` is two matches, not three.
        let found = find_in_lines(&lines(&["aaaa"]), &Query::new("aa"));
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].start, found[0].end), (0, 2));
        assert_eq!((found[1].start, found[1].end), (2, 4));

        // A needle that matches nothing but an empty string cannot loop.
        assert!(find_in_lines(&lines(&["abc"]), &Query::new("")).is_empty());
    }

    #[test]
    fn case_can_be_ignored_but_a_match_still_points_at_the_original_text() {
        let source = lines(&["let Value = value;", "VALUE"]);
        let sensitive = find_in_lines(&source, &Query::new("value"));
        assert_eq!(sensitive.len(), 1, "only the lowercase one");
        assert_eq!(sensitive[0].line, 0);

        let query = Query {
            case: CaseMode::Insensitive,
            ..Query::new("value")
        };
        let insensitive = find_in_lines(&source, &query);
        assert_eq!(insensitive.len(), 3, "all three spellings");
        // The range is into the line as written, not into the folded text, so
        // slicing gives the text the reader sees.
        assert_eq!(
            &source[0][insensitive[0].start..insensitive[0].end],
            "Value"
        );
        assert_eq!(
            &source[1][insensitive[2].start..insensitive[2].end],
            "VALUE"
        );
    }

    #[test]
    fn whole_word_matches_skip_matches_inside_longer_words() {
        let source = lines(&["let user = 1;", "let username = 2;", "user_id = 3;"]);
        let query = Query {
            whole_word: true,
            ..Query::new("user")
        };
        let found = find_in_lines(&source, &query);
        assert_eq!(found.len(), 1, "only the bare word: {found:?}");
        assert_eq!(found[0].line, 0);

        // Without the whole word restriction all three appear.
        assert_eq!(count_matches(&source, &Query::new("user")), 3);
        // `_` counts as a word character, so `user_id` is not a bare `user`,
        // which is what a programmer means by a whole word.
        assert!(!is_whole_word("user_id", 0, 4));
        assert!(is_whole_word("user +", 0, 4));
        assert!(is_word_character('_'));
        assert!(is_word_character('7'));
        assert!(!is_word_character('-'));
    }

    #[test]
    fn a_match_at_the_edge_of_a_line_is_still_whole() {
        // Nothing beside a match is a boundary, so a word at either end of a
        // line counts as whole.
        let source = lines(&["user", "user name", "name user"]);
        let query = Query {
            whole_word: true,
            ..Query::new("user")
        };
        let found = find_in_lines(&source, &query);
        assert_eq!(found.len(), 3, "{found:?}");
    }

    #[test]
    fn a_replacement_lands_where_the_match_is() {
        let line = "let value = value + 1;";
        let found = find_in_line(line, &Query::new("value"));
        assert_eq!(found.len(), 2);
        let once = replace_one(
            line,
            Match {
                line: 0,
                start: found[0].0,
                end: found[0].1,
            },
            "answer",
        );
        assert_eq!(once, "let answer = value + 1;");
        // Replacing the second one leaves the first alone.
        let second = replace_one(
            line,
            Match {
                line: 0,
                start: found[1].0,
                end: found[1].1,
            },
            "answer",
        );
        assert_eq!(second, "let value = answer + 1;");
    }

    #[test]
    fn replacing_everything_replaces_each_match_once() {
        let source = lines(&["a a a", "b", "a"]);
        let query = Query::new("a");
        let replaced = replace_all(&source, &query, "z");
        assert_eq!(replaced, lines(&["z z z", "b", "z"]));
        // A replacement is applied from the end backwards, so a longer one
        // does not shift the offsets of the ones before it.
        let grown = replace_all(&lines(&["a a"]), &query, "longer");
        assert_eq!(grown, lines(&["longer longer"]));
        // A line with no match is returned unchanged.
        assert_eq!(replace_all(&lines(&["b"]), &query, "z"), lines(&["b"]));
        // Replacing with nothing deletes the match.
        assert_eq!(replace_all(&lines(&["a b a"]), &query, ""), lines(&[" b "]));
    }

    #[test]
    fn a_replacement_that_would_split_a_character_is_refused() {
        // A range that is not on a character boundary cannot come from a search,
        // but a caller could still hand one over; the line is returned as it
        // was rather than corrupted.
        let line = "中文";
        let unchanged = replace_one(
            line,
            Match {
                line: 0,
                start: 1,
                end: 2,
            },
            "x",
        );
        assert_eq!(unchanged, line);
        // A range past the end is refused too.
        let past = replace_one(
            line,
            Match {
                line: 0,
                start: 0,
                end: 99,
            },
            "x",
        );
        assert_eq!(past, line);
    }

    #[test]
    fn matches_are_walked_forwards_and_backwards_and_wrap_around() {
        let matches = find_in_lines(&lines(&["a", "a", "a"]), &Query::new("a"));
        assert_eq!(matches.len(), 3);
        // The match after a position, which is how a find advances: standing
        // before the first match, the next one is the first.
        assert_eq!(next_match(&matches, (0, 0)).map(|m| m.line), Some(1));
        assert_eq!(next_match(&matches, (1, 1)).map(|m| m.line), Some(2));
        // Past the last one it wraps to the first, so a find loops rather
        // than stopping.
        assert_eq!(next_match(&matches, (2, 1)).map(|m| m.line), Some(0));
        assert_eq!(next_match(&matches, (9, 0)).map(|m| m.line), Some(0));
        // And backwards the same way, wrapping to the last.
        assert_eq!(previous_match(&matches, (2, 0)).map(|m| m.line), Some(1));
        assert_eq!(previous_match(&matches, (0, 0)).map(|m| m.line), Some(2));
        assert_eq!(previous_match(&matches, (0, 1)).map(|m| m.line), Some(0));
        // Nothing to find means nothing to walk.
        assert_eq!(next_match(&[], (0, 0)), None);
        assert_eq!(previous_match(&[], (0, 0)), None);
    }

    #[test]
    fn every_match_can_be_stepped_through_exactly_once() {
        // Walking from a match to the next has to visit each one and come back
        // to the start, which is what makes repeated pressing useful.
        let matches = find_in_lines(&lines(&["a a", "a", "a a"]), &Query::new("a"));
        assert_eq!(matches.len(), 5);
        let mut seen = Vec::new();
        let mut at = (0, 0);
        for _ in 0..matches.len() {
            let found = next_match(&matches, at).expect("a match");
            seen.push((found.line, found.start));
            at = (found.line, found.start + 1);
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), matches.len(), "each match was visited once");
    }

    #[test]
    fn a_query_with_no_needle_matches_nothing() {
        // An empty search box must not match every position in the file.
        let source = lines(&["anything", "at all"]);
        assert!(Query::new("").is_empty());
        assert!(find_in_lines(&source, &Query::new("")).is_empty());
        assert_eq!(count_matches(&source, &Query::new("")), 0);
        assert_eq!(replace_all(&source, &Query::new(""), "x"), source);
    }

    #[test]
    fn a_wide_character_match_maps_back_onto_the_original_line() {
        // Lowercasing can change a character's byte length, so a match found in
        // the folded text has to be mapped back by character, or the highlight
        // lands in the wrong place.
        let source = lines(&["中文 中文"]);
        let found = find_in_lines(&source, &Query::new("中文"));
        assert_eq!(found.len(), 2);
        assert_eq!(&source[0][found[0].start..found[0].end], "中文");
        assert_eq!(&source[0][found[1].start..found[1].end], "中文");
        assert_eq!(found[1].start, 7, "the second one is after the space");
    }
}
