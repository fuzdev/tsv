//! The memo of the TypeScript grammar's answers to the raw scans — `closes_type_arguments`
//! and `closes_statement_header` (`crate::__private::OPERAND_GRAMMAR`) — kept for a whole
//! parse or print: its roots ([`remembering`], [`Root`]), its address-range guard, and the
//! lookup the two answers go through ([`answer`]).
//!
//! Each answer walks its region again, so a nest of templates would ask the questions
//! inside it once per level around them, exponential in the nesting; and the parser starts a
//! raw lookahead at each arrow head, type-argument list and paren group it meets, each of
//! which asks the same answers about the levels inside it, so a memo dropped with each scan
//! works every answer out again once per scan around it. Remembered for the whole parse, each
//! is worked out once and a nest costs a low polynomial in its depth.
//!
//! The answers read nothing from it: each is a walk the memo only stands in for, and
//! answers the same without it.
// TODO: the memo stands in for linear answers. A forward scan that recorded each
// operand-position `<` (and statement header's `)`) as it passed would answer
// `closes_type_arguments` and `closes_statement_header` from that record in one pass. This
// module would then be deleted with its call sites: the two `answer` wrappers in
// `expression_type_args` (which call their walks directly), the root in
// `parse_retrying_acorn_followers` and in `scan::SkippedTemplate`, the three print roots in
// `lib.rs` (`with_doc_printer`, `with_program_printer`, `format_program_in`), and
// `crate::__private::remembering_grammar_answers` with its one caller, `tsv_svelte`'s
// `parse_svelte`; and `ScanStart`'s `Hash` derive, which only the memo key uses (public in
// `tsv_lang`: keep it, or drop it as a break).

use std::cell::{Cell, RefCell};
use std::marker::PhantomData;

use tsv_lang::FxHashMap;
use tsv_lang::source_scan::ScanStart;

/// Run `parse` — a parse of `source`, or any other root of the raw scans over it — with the
/// grammar's answers about `source` remembered until it returns ([`answer`]).
///
/// A root opened inside another over the same bytes, or a slice of them, joins it, so the
/// outermost root's memo serves the whole document. One opened over other bytes — a
/// temporary buffer parsed mid-parse — keeps a memo of its own until it returns, and the
/// outer one is restored then: **the memo's address key is sound only because every answer
/// it holds is about the one source its root borrows**, whose bytes cannot move or change
/// while the root is open ([`Root`] holds that range, and keeps no answer about bytes
/// outside it).
pub(crate) fn remembering<R>(source: &[u8], parse: impl FnOnce() -> R) -> R {
    let _root = Root::enter(source);
    parse()
}

/// The grammar's questions the memo holds the answers to.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Question {
    /// `closes_type_arguments`: the `>` at the key's `at`.
    TypeArguments,
    /// `closes_statement_header`: the `)` at the key's `at`.
    StatementHeader,
}

/// Everything one of the grammar's answers reads: the question, the bytes asked about (by
/// address and length — inside the root's source, which stays put while the memo lives),
/// the delimiter's offset, the walk's `lower_bound` and how its region begins.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    question: Question,
    bytes: (usize, usize),
    at: usize,
    lower_bound: usize,
    start: ScanStart,
}

/// The address range of a root's source: its first byte and one past its last.
type SourceRange = (usize, usize);

type Memo = FxHashMap<Key, Option<bool>>;

/// This thread's memo and the root it belongs to.
struct Answers {
    /// The source the open root borrows, whose bytes alone the memo holds answers about;
    /// `None` with no root open.
    root: Cell<Option<SourceRange>>,
    /// The answers given since the root opened; built at the first answer kept.
    memo: RefCell<Option<Memo>>,
}

thread_local! {
    static ANSWERS: Answers = const {
        Answers {
            root: Cell::new(None),
            memo: RefCell::new(None),
        }
    };
}

/// Answer one of the grammar's questions by `walk` — a walk from `lower_bound` to the
/// delimiter at `at`, of a region that begins as `start` says — or from the memo of the root
/// open over `bytes` ([`remembering`]).
///
/// The memo is sound because an answer reads nothing but its key: every walk reads only the
/// bytes from `lower_bound` up to `at`, under the one grammar, from fresh state — no
/// function context, grouping depth or flag of the caller's. The one exception is a template
/// nested past `tsv_lang::NESTING_LIMIT` (`scan::skip_template_in_grammar`), which the
/// parser refuses for its nesting whatever the answers. An answer asked with no root open
/// over `bytes` is a root of its own: the answers inside it are remembered until it returns,
/// and it is not kept.
pub(super) fn answer(
    question: Question,
    bytes: &[u8],
    at: usize,
    lower_bound: usize,
    start: ScanStart,
    walk: impl FnOnce() -> Option<bool>,
) -> Option<bool> {
    let key = Key {
        question,
        bytes: (bytes.as_ptr() as usize, bytes.len()),
        at,
        lower_bound,
        start,
    };
    let root = Root::enter(bytes);
    let kept = !root.opened();
    if kept && let Some(known) = remembered(key) {
        return known;
    }
    let answered = walk();
    if kept {
        remember(key, answered);
    }
    answered
}

/// The answer remembered for `key`, which is itself an `Option` (`None`: the grammar cannot
/// tell); `None` when none is.
#[expect(
    clippy::option_option,
    reason = "a memo miss beside a remembered answer that is itself `None`"
)]
fn remembered(key: Key) -> Option<Option<bool>> {
    ANSWERS
        .try_with(|answers| answers.memo.borrow().as_ref()?.get(&key).copied())
        .ok()
        .flatten()
}

fn remember(key: Key, answered: Option<bool>) {
    // A thread whose locals are being torn down keeps no memo, and answers each again.
    let _ = ANSWERS.try_with(|answers| {
        answers
            .memo
            .borrow_mut()
            .get_or_insert_with(FxHashMap::default)
            .insert(key, answered);
    });
}

/// One open root of the memo over the source it borrows, until dropped.
///
/// Entered inside a root whose source holds its own, it joins that root and its drop does
/// nothing. Otherwise it is a root: it sets the memo aside with the range it was about, starts
/// an empty one about its own source, and its drop drops that memo and puts the outer one
/// back — unwinding included, so nothing carries from one parse to the next on a thread.
/// Roots are held only as locals of the scopes they cover ([`remembering`], the parse driver,
/// the template skip, [`answer`]), so they close in the reverse order they opened.
pub(super) struct Root<'s> {
    /// The memo and range this root set aside, when it is a root — `None` when it joined the
    /// one open, or the thread's locals are being torn down and it keeps no memo.
    outer: Option<Outer>,
    _source: PhantomData<&'s [u8]>,
}

/// What a [`Root`] set aside to open: the root open before it and that root's memo.
struct Outer {
    root: Option<SourceRange>,
    memo: Option<Memo>,
}

impl<'s> Root<'s> {
    /// Open a root over `source`, or join the one open over it; held as a local of the
    /// scope it covers. Allocates nothing: the memo is built at its first answer.
    #[inline]
    pub(super) fn enter(source: &'s [u8]) -> Self {
        let first = source.as_ptr() as usize;
        let range = (first, first + source.len());
        let outer = ANSWERS
            .try_with(|answers| {
                let open = answers.root.get();
                if open.is_some_and(|(first, end)| first <= range.0 && range.1 <= end) {
                    return None;
                }
                Some(Outer {
                    root: answers.root.replace(Some(range)),
                    memo: answers.memo.take(),
                })
            })
            .ok()
            .flatten();
        Self {
            outer,
            _source: PhantomData,
        }
    }

    /// Whether this is the root its memo began with, rather than one that joined it.
    #[inline]
    fn opened(&self) -> bool {
        self.outer.is_some()
    }
}

impl Drop for Root<'_> {
    #[inline]
    fn drop(&mut self) {
        if let Some(outer) = self.outer.take() {
            let _ = ANSWERS.try_with(|answers| {
                answers.root.set(outer.root);
                answers.memo.replace(outer.memo)
            });
        }
    }
}
