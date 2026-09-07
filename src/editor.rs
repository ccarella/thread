//! Char-safe cursor over a `String`. The cursor is a Unicode scalar index,
//! never a raw byte offset, so edits do not panic on UTF-8.

pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

pub fn byte_of(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

pub fn clamp(s: &str, cursor: usize) -> usize {
    cursor.min(char_len(s))
}

/// `(line, column, line_start)` in scalar indices. Lines are `\n`-separated.
pub fn line_col(s: &str, cursor: usize) -> (usize, usize, usize) {
    let cursor = clamp(s, cursor);
    let mut line = 0usize;
    let mut start = 0usize;
    let mut idx = 0usize;
    for ch in s.chars() {
        if idx == cursor {
            return (line, idx - start, start);
        }
        idx += 1;
        if ch == '\n' {
            line += 1;
            start = idx;
        }
    }
    (line, idx - start, start)
}

fn line_starts(s: &str) -> Vec<usize> {
    let mut starts = vec![0];
    let mut idx = 0usize;
    for ch in s.chars() {
        idx += 1;
        if ch == '\n' {
            starts.push(idx);
        }
    }
    starts
}

fn line_end(s: &str, start: usize) -> usize {
    let mut idx = start;
    for ch in s.chars().skip(start) {
        if ch == '\n' {
            return idx;
        }
        idx += 1;
    }
    idx
}

pub fn insert_char(s: &mut String, cursor: &mut usize, c: char) {
    *cursor = clamp(s, *cursor);
    let i = byte_of(s, *cursor);
    s.insert(i, c);
    *cursor += 1;
}

pub fn backspace(s: &mut String, cursor: &mut usize) {
    *cursor = clamp(s, *cursor);
    if *cursor == 0 {
        return;
    }
    *cursor -= 1;
    let i = byte_of(s, *cursor);
    s.remove(i);
}

pub fn move_left(s: &str, cursor: &mut usize) {
    *cursor = clamp(s, *cursor).saturating_sub(1);
}

pub fn move_right(s: &str, cursor: &mut usize) {
    *cursor = clamp(s, *cursor).saturating_add(1).min(char_len(s));
}

pub fn home(s: &str, cursor: &mut usize) {
    let (_, _, start) = line_col(s, *cursor);
    *cursor = start;
}

pub fn end(s: &str, cursor: &mut usize) {
    let (_, _, start) = line_col(s, *cursor);
    *cursor = line_end(s, start);
}

pub fn move_up(s: &str, cursor: &mut usize) {
    let (line, col, _) = line_col(s, *cursor);
    if line == 0 {
        *cursor = 0;
        return;
    }
    let starts = line_starts(s);
    let start = starts[line - 1];
    let end = line_end(s, start);
    *cursor = start + col.min(end - start);
}

pub fn move_down(s: &str, cursor: &mut usize) {
    let (line, col, _) = line_col(s, *cursor);
    let starts = line_starts(s);
    if line + 1 >= starts.len() {
        *cursor = char_len(s);
        return;
    }
    let start = starts[line + 1];
    let end = line_end(s, start);
    *cursor = start + col.min(end - start);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_insert_and_backspace_do_not_panic() {
        let mut s = String::from("aé本");
        let mut cursor = char_len(&s);
        insert_char(&mut s, &mut cursor, '🎉');
        assert_eq!(s, "aé本🎉");
        assert_eq!(cursor, 4);
        backspace(&mut s, &mut cursor);
        assert_eq!(s, "aé本");
        assert_eq!(cursor, 3);
        home(&s, &mut cursor);
        assert_eq!(cursor, 0);
        backspace(&mut s, &mut cursor);
        assert_eq!(s, "aé本");
    }

    #[test]
    fn arrows_and_home_end_move_by_char_and_line() {
        let s = String::from("ab\nçd");
        let mut cursor = 0;
        end(&s, &mut cursor);
        assert_eq!(cursor, 2);
        move_right(&s, &mut cursor);
        assert_eq!(cursor, 3);
        move_down(&s, &mut cursor);
        assert_eq!(cursor, char_len(&s));
        home(&s, &mut cursor);
        assert_eq!(cursor, 3);
        move_up(&s, &mut cursor);
        assert_eq!(cursor, 0);
        move_left(&s, &mut cursor);
        assert_eq!(cursor, 0);
    }

    #[test]
    fn insert_in_the_middle_is_char_safe() {
        let mut s = String::from("a🎉c");
        let mut cursor = 1;
        insert_char(&mut s, &mut cursor, 'b');
        assert_eq!(s, "ab🎉c");
        assert_eq!(cursor, 2);
    }
}
