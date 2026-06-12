// Source-scanning utilities for finding characters while skipping comments.
//
// These are used by both the AST conversion layer (acorn comment duplication)
// and the printer (finding delimiters like `:`, `[`, `]`, `(`).

/// Skip over a comment (line or block) starting at position `i`.
///
/// Returns `Some(new_i)` where `new_i` is the position AFTER the comment
/// (ready for the next iteration), or `None` if not at a comment.
pub fn skip_comment(bytes: &[u8], i: usize, end: usize) -> Option<usize> {
    if i + 1 >= end || bytes[i] != b'/' {
        return None;
    }
    if bytes[i + 1] == b'/' {
        // Line comment - skip to end of line
        let mut j = i + 2;
        while j < end && bytes[j] != b'\n' {
            j += 1;
        }
        Some(j)
    } else if bytes[i + 1] == b'*' {
        // Block comment - skip to */
        let mut j = i + 2;
        while j + 1 < end && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
            j += 1;
        }
        Some(j + 2) // Past the */
    } else {
        None
    }
}

/// Find the first occurrence of a byte in source between `start` and `end`, skipping comments.
///
/// Returns the position of the byte, or `None` if not found.
pub fn find_char_skipping_comments(
    bytes: &[u8],
    start: usize,
    end: usize,
    target: u8,
) -> Option<usize> {
    let mut i = start;
    while i < end {
        if let Some(new_i) = skip_comment(bytes, i, end) {
            i = new_i;
            continue;
        }
        if bytes[i] == target {
            return Some(i);
        }
        i += 1;
    }
    None
}
