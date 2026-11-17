//! [`value_runs!`](crate::value_runs!), the declaration of a language crate's value-run stores,
//! and [`FieldShape`], what a test reads of one store, scratch stack or pool.
//!
//! A value run is a list whose elements are built whole and stand nowhere else: a
//! [`Run`](crate::store::Run) over a [`Store`] of the element type, built on that type's
//! scratch stack (a [`ListBuf`]) and flushed into the store when the list closes. A language
//! crate keeps one store and one scratch stack per element type, and each pair takes the same
//! lines: a field of each, their place in the constructor, the reset, the scratch marks and
//! the clears, and the view accessor that reads one list. The macro writes those lines from
//! one entry per element type, so the crates that hold such lists declare them alike and none
//! copies the plumbing.

use crate::store::{ListBuf, Row, Store, TextPool};

/// Declares a language crate's value-run stores: a struct holding one [`Store`] and one
/// scratch stack ([`ListBuf`]) per element type, a struct of the scratch stacks' marks, and
/// the accessors that read a list through the crate's view.
///
/// ```ignore
/// tsv_lang::value_runs! {
///     /// The stores of the lists built by value.
///     pub(crate) struct RunStores;
///     /// The tops of their scratch stacks.
///     pub(crate) struct RunMarks;
///     pub view AstView::runs;
///
///     /// Declarators, the `a = 1` of `let a = 1`.
///     declarators, declarator_scratch: VariableDeclarator => declarators;
///     /// The spans of a selector's parts.
///     part_spans, part_span_scratch: Span as Row => part_spans;
///     /// Tokens no reader asks for.
///     tokens, token_scratch: Token;
/// }
/// ```
///
/// The header names what the caller chooses:
///
/// - the stores struct, with its visibility, which its fields and methods take too. The parser
///   reads the fields by path (`stores.runs.declarator_scratch.push(row)`), so a flush can
///   borrow a scratch stack and its store apart from every other field;
/// - the marks struct, with its visibility. Its fields are private: a mark is taken by the
///   stores' `marks` and spent by its `truncate`;
/// - the view the accessors are methods of, with their visibility, and the view's method that
///   returns the stores struct for the view's lifetime (`AstView::runs`, here
///   `fn runs(self) -> &'a RunStores`). The view is a type of the calling crate, named bare
///   (the accessors are an inherent `impl` of it), `Copy`, with one lifetime parameter and no
///   other.
///
/// Each entry names a store field, its scratch stack's field and the element type; its doc
/// comment documents the store field. An element type has one entry: a second store of the
/// same type would mint runs the first one's accessor also takes. An attribute on an entry
/// goes on the store field alone, so an entry cannot be `cfg`-gated. Two parts of an entry
/// are optional:
///
/// - `as Row` after the type says the type states its family itself, so the macro writes no
///   [`Row`] impl for it: a type another crate owns (a span, an id, a pool handle), or one
///   that is a row of another store too. Without it the macro makes the type its own family
///   (`impl Row for T { type Family = Self; }`), so its lists are `Run<T>`;
/// - `=> name` declares the view accessor `name`, which reads one list as a slice. Without it
///   no reader reaches the store through the view.
///
/// The stores struct gets `new` (allocating nothing), `reset` (every store and scratch stack
/// rewound in place, capacity kept), `marks` and `truncate` (the scratch stacks' tops, taken
/// into a caller's `MarkGroup` and rewound to), `clear_scratch`, and, in a build with debug
/// assertions, `scratch_is_empty`. A test build of the caller also gets `shapes`, every
/// field's [`FieldShape`] in declaration order, and `FIELDS`, the store fields' names. The
/// macro places no bound on an element type beyond [`Row`]: which flush a list takes
/// (`ListBuf::flush_copy` or `flush_copy_short`) is the choice of the code that builds it.
///
/// A scratch stack with no store (a buffer framed and truncated, never flushed) is not a value
/// run and has no entry: it stays a field of the caller's own store struct, beside the
/// generated one, with its line in that struct's reset, marks and clears.
///
/// Every path the expansion names goes through `$crate`, so a caller imports nothing for it.
/// The compiler reports no unused-item lint on what another crate's macro expands to: a
/// generated method the caller no longer calls (the `truncate` its own rewind forwards to,
/// say), an entry nothing reads, or tests that leave `shapes` or `FIELDS` unread are not
/// flagged. The caller's tests stand in for the lint: a fill that leaves every field of
/// `shapes` written, checked against `FIELDS`, and a test through each of the caller's own
/// wrappers (its reset, its marks and rewind, its clears) that reads every field after it.
#[doc(hidden)]
#[macro_export]
macro_rules! value_runs {
    (
        $(#[$stores_meta:meta])*
        $vis:vis struct $Stores:ident;
        $(#[$marks_meta:meta])*
        $marks_vis:vis struct $Marks:ident;
        $acc_vis:vis view $View:ident :: $reach:ident;
        $(
            $(#[$m:meta])*
            $field:ident, $scratch:ident : $Row:ty $(as $stated:ident)? $(=> $acc:ident)?;
        )*
    ) => {
        $(
            $crate::__value_runs_family!($Row $(, $stated)?);
        )*

        $(#[$stores_meta])*
        $vis struct $Stores {
            $(
                $(#[$m])*
                $vis $field: $crate::store::Store<$Row>,
                #[doc = concat!("The scratch stack that builds the lists of `", stringify!($field), "`.")]
                $vis $scratch: $crate::store::ListBuf<$Row>,
            )*
        }

        $(#[$marks_meta])*
        #[derive(Clone, Copy)]
        $marks_vis struct $Marks {
            $($field: $crate::store::ListMark<<$Row as $crate::store::Row>::Family>,)*
        }

        impl $Stores {
            /// Empty stores, allocating nothing.
            $vis fn new() -> Self {
                Self {
                    $(
                        $field: $crate::store::Store::new(),
                        $scratch: $crate::store::ListBuf::new(),
                    )*
                }
            }

            /// Rewinds the stores and scratch stacks in place, keeping each written store's
            /// slot 0 and every allocation's capacity.
            #[inline]
            $vis fn reset(&mut self) {
                $(
                    self.$field.reset();
                    self.$scratch.reset();
                )*
            }

            /// Every store's and scratch stack's shape, for the tests that read what a reset
            /// leaves.
            #[cfg(test)]
            $vis fn shapes(&self, out: &mut ::std::vec::Vec<$crate::value_runs::FieldShape>) {
                $(
                    out.push($crate::value_runs::FieldShape::store(stringify!($field), &self.$field));
                    out.push($crate::value_runs::FieldShape::scratch(
                        stringify!($scratch),
                        &self.$scratch,
                    ));
                )*
            }

            /// The current tops of the scratch stacks, their heights left for `group` to check.
            #[inline]
            $vis fn marks(&self, group: &mut $crate::store::MarkGroup) -> $Marks {
                $Marks {
                    $($field: self.$scratch.mark_in(group),)*
                }
            }

            /// Truncates the scratch stacks to `marks`, discarding the frames opened since.
            #[inline]
            $vis fn truncate(&mut self, marks: &$Marks) {
                $(self.$scratch.truncate(marks.$field);)*
            }

            /// Empties the scratch stacks, discarding every open frame. The stores keep their
            /// rows.
            #[inline]
            $vis fn clear_scratch(&mut self) {
                $(self.$scratch.reset();)*
            }

            /// Whether every scratch stack is empty.
            #[cfg(debug_assertions)]
            $vis fn scratch_is_empty(&self) -> bool {
                true $(&& self.$scratch.is_empty())*
            }

            /// Every store's field name, for the test that walks them all.
            #[cfg(test)]
            $vis const FIELDS: &'static [&'static str] = &[$(stringify!($field)),*];
        }

        impl<'a> $View<'a> {
            $($(
                #[doc = concat!(
                    "The [`", stringify!($Row), "`] rows of `run`, one range check for the whole ",
                    "run. A miss ends in the store's shared cold path."
                )]
                #[inline]
                $acc_vis fn $acc(
                    self,
                    run: $crate::store::Run<<$Row as $crate::store::Row>::Family>,
                ) -> &'a [$Row] {
                    self.$reach().$field.slice(run)
                }
            )?)*
        }
    };
}

/// [`value_runs!`](crate::value_runs!)'s statement of one element type's family: the type as
/// its own family, or nothing for a type an entry marks `as Row`, which states it elsewhere.
#[doc(hidden)]
#[macro_export]
macro_rules! __value_runs_family {
    ($Row:ty) => {
        impl $crate::store::Row for $Row {
            type Family = Self;
        }
    };
    ($Row:ty, Row) => {};
}

/// What a test reads of one store, scratch stack or pool: its length and its capacity.
///
/// A store set lists one per field (the `shapes` a [`value_runs!`](crate::value_runs!) struct
/// generates, and the caller's own lines for the fields it declares by hand), and its tests
/// compare the list across a reset, a clear or a park.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldShape {
    /// The field's name.
    pub name: &'static str,
    /// Which kind of buffer the field is.
    pub kind: FieldKind,
    /// The slots, elements or bytes the buffer holds.
    pub len: usize,
    /// The slots, elements or bytes the buffer's allocation holds.
    pub capacity: usize,
}

/// Which kind of buffer a [`FieldShape`] reads, which decides its rewound length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// A [`Store`].
    Store,
    /// A scratch stack, a [`ListBuf`].
    Scratch,
    /// A [`TextPool`].
    Pool,
}

impl FieldShape {
    /// The shape of `store`, the field `name`.
    pub fn store<T: Row>(name: &'static str, store: &Store<T>) -> Self {
        Self {
            name,
            kind: FieldKind::Store,
            len: store.len(),
            capacity: store.capacity(),
        }
    }

    /// The shape of `scratch`, the field `name`.
    pub fn scratch<T>(name: &'static str, scratch: &ListBuf<T>) -> Self {
        Self {
            name,
            kind: FieldKind::Scratch,
            len: scratch.len(),
            capacity: scratch.capacity(),
        }
    }

    /// The shape of `pool`, the field `name`.
    pub fn pool<F>(name: &'static str, pool: &TextPool<F>) -> Self {
        Self {
            name,
            kind: FieldKind::Pool,
            len: pool.len(),
            capacity: pool.capacity(),
        }
    }

    /// The length a reset leaves: slot 0 of a store and the sentinel of a pool once written,
    /// nothing of a scratch stack.
    pub fn rewound_len(self) -> usize {
        match self.kind {
            FieldKind::Store | FieldKind::Pool => self.len.min(1),
            FieldKind::Scratch => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FieldKind, FieldShape};
    use crate::Span;
    use crate::store::{MarkGroup, PoolText, Run, TextPool};

    /// The family of the toy pool.
    enum Words {}

    /// A `Copy` element, flushed as a block.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Leaf(u32);

    /// An element holding a list of its own type and a string of the pool.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Branch {
        name: PoolText<Words>,
        children: Run<Branch>,
        leaves: Run<Leaf>,
    }

    /// An element no reader reaches through the view.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Token(u8);

    /// A toy document's stores: the generated struct beside a field declared by hand.
    struct ToyStores {
        lists: ToyLists,
        text: TextPool<Words>,
    }

    #[derive(Clone, Copy)]
    struct ToyView<'a> {
        stores: &'a ToyStores,
    }

    impl<'a> ToyView<'a> {
        fn lists(self) -> &'a ToyLists {
            &self.stores.lists
        }

        fn text(self, text: PoolText<Words>) -> &'a str {
            self.stores.text.get(text)
        }
    }

    crate::value_runs! {
        /// The toy stores: private, where a language crate's are crate-visible.
        struct ToyLists;
        /// The tops of the toy scratch stacks.
        struct ToyMarks;
        view ToyView::lists;

        /// `Copy` rows, read through the view.
        leaves, leaf_scratch: Leaf => leaves;
        /// Rows each holding a list of its own type.
        branches, branch_scratch: Branch => branches;
        /// Rows with no accessor.
        tokens, token_scratch: Token;
        /// Rows whose type states its family itself.
        spans, span_scratch: Span as Row => spans;
        /// Rows that are handles of the pool, a generic type whose family is stated beside it.
        names, name_scratch: PoolText<Words> as Row => names;
    }

    impl ToyStores {
        fn new() -> Self {
            Self {
                lists: ToyLists::new(),
                text: TextPool::new(),
            }
        }

        fn view(&self) -> ToyView<'_> {
            ToyView { stores: self }
        }

        fn shapes(&self) -> Vec<FieldShape> {
            let mut out = vec![FieldShape::pool("text", &self.text)];
            self.lists.shapes(&mut out);
            out
        }
    }

    fn span(start: u32) -> Span {
        Span {
            start,
            end: start + 1,
        }
    }

    fn branch(stores: &mut ToyStores, name: &str, children: Run<Branch>) -> Branch {
        Branch {
            name: stores.text.push(name),
            children,
            leaves: Run::EMPTY,
        }
    }

    fn names(stores: &ToyStores, run: Run<Branch>) -> Vec<&str> {
        let view = stores.view();
        view.branches(run)
            .iter()
            .map(|branch| view.text(branch.name))
            .collect()
    }

    /// Writes every store, leaves one element on every scratch stack, and writes the pool.
    fn fill(stores: &mut ToyStores) {
        let lists = &mut stores.lists;
        for round in 0..40 {
            let mark = lists.leaf_scratch.mark();
            lists.leaf_scratch.push(Leaf(round));
            lists.leaf_scratch.push(Leaf(round + 1));
            lists.leaf_scratch.flush_copy(mark, &mut lists.leaves);
            let mark = lists.token_scratch.mark();
            lists.token_scratch.push(Token(1));
            lists
                .token_scratch
                .flush_copy_short(mark, &mut lists.tokens);
            let mark = lists.span_scratch.mark();
            lists.span_scratch.push(span(round));
            lists.span_scratch.flush_copy_short(mark, &mut lists.spans);
        }
        let name = stores.text.push("a name");
        let mark = stores.lists.name_scratch.mark();
        stores.lists.name_scratch.push(name);
        stores
            .lists
            .name_scratch
            .flush_copy_short(mark, &mut stores.lists.names);
        stores.lists.name_scratch.push(name);
        let mark = stores.lists.branch_scratch.mark();
        let row = branch(stores, "filled", Run::EMPTY);
        stores.lists.branch_scratch.push(row);
        stores
            .lists
            .branch_scratch
            .flush_copy(mark, &mut stores.lists.branches);
        stores.lists.leaf_scratch.push(Leaf(0));
        stores.lists.branch_scratch.push(row);
        stores.lists.token_scratch.push(Token(0));
        stores.lists.span_scratch.push(span(0));
    }

    fn assert_every_field_written(filled: &[FieldShape]) {
        let unwritten: Vec<_> = filled
            .iter()
            .filter(|shape| match shape.kind {
                FieldKind::Store | FieldKind::Pool => shape.len <= 1,
                FieldKind::Scratch => shape.len == 0,
            })
            .map(|shape| shape.name)
            .collect();
        assert!(unwritten.is_empty(), "left unwritten: {unwritten:?}");
    }

    #[test]
    fn the_fields_are_declared_in_entry_order() {
        assert_eq!(
            ToyLists::FIELDS,
            ["leaves", "branches", "tokens", "spans", "names"]
        );
        let stores = ToyStores::new();
        let shapes = stores.shapes();
        let named: Vec<_> = shapes
            .iter()
            .map(|shape| (shape.name, shape.kind))
            .collect();
        assert_eq!(
            named,
            [
                ("text", FieldKind::Pool),
                ("leaves", FieldKind::Store),
                ("leaf_scratch", FieldKind::Scratch),
                ("branches", FieldKind::Store),
                ("branch_scratch", FieldKind::Scratch),
                ("tokens", FieldKind::Store),
                ("token_scratch", FieldKind::Scratch),
                ("spans", FieldKind::Store),
                ("span_scratch", FieldKind::Scratch),
                ("names", FieldKind::Store),
                ("name_scratch", FieldKind::Scratch),
            ]
        );
    }

    #[test]
    fn new_stores_allocate_nothing() {
        let stores = ToyStores::new();
        for shape in stores.shapes() {
            assert_eq!((shape.len, shape.capacity), (0, 0), "{}", shape.name);
        }
    }

    /// Lists of one element type nest on one scratch stack: an inner list opened while the
    /// outer frame holds elements flushes as its own contiguous run, and the outer frame
    /// flushes around it.
    #[test]
    fn nested_lists_of_one_type_interleave_and_flush_as_contiguous_runs() {
        let mut stores = ToyStores::new();
        let outer = stores.lists.branch_scratch.mark();
        let a = branch(&mut stores, "a", Run::EMPTY);
        stores.lists.branch_scratch.push(a);

        let inner = stores.lists.branch_scratch.mark();
        let b1 = branch(&mut stores, "b1", Run::EMPTY);
        stores.lists.branch_scratch.push(b1);
        let innermost = stores.lists.branch_scratch.mark();
        let c1 = branch(&mut stores, "c1", Run::EMPTY);
        stores.lists.branch_scratch.push(c1);
        let c2 = branch(&mut stores, "c2", Run::EMPTY);
        stores.lists.branch_scratch.push(c2);
        let grandchildren = stores
            .lists
            .branch_scratch
            .flush_copy(innermost, &mut stores.lists.branches);
        let b2 = branch(&mut stores, "b2", grandchildren);
        stores.lists.branch_scratch.push(b2);
        let children = stores
            .lists
            .branch_scratch
            .flush_copy(inner, &mut stores.lists.branches);

        let b = branch(&mut stores, "b", children);
        stores.lists.branch_scratch.push(b);
        let top = stores
            .lists
            .branch_scratch
            .flush_copy(outer, &mut stores.lists.branches);

        assert_eq!(names(&stores, top), ["a", "b"]);
        assert_eq!(names(&stores, children), ["b1", "b2"]);
        assert_eq!(names(&stores, grandchildren), ["c1", "c2"]);
        let view = stores.view();
        assert!(view.branches(view.branches(top)[0].children).is_empty());
        assert_eq!(format!("{grandchildren:?}"), "Run(1, 2)");
        assert_eq!(format!("{children:?}"), "Run(3, 2)");
        assert_eq!(format!("{top:?}"), "Run(5, 2)");
        assert_eq!(stores.lists.branch_scratch.len(), 0);
    }

    #[test]
    fn an_empty_list_leaves_its_store_unwritten() {
        let mut stores = ToyStores::new();
        let mark = stores.lists.leaf_scratch.mark();
        let run = stores
            .lists
            .leaf_scratch
            .flush_copy_short(mark, &mut stores.lists.leaves);
        assert_eq!(run, Run::EMPTY);
        assert!(stores.view().leaves(run).is_empty());
        assert_eq!(stores.lists.leaves.len(), 0, "nothing primed");
        assert_eq!(stores.lists.leaves.capacity(), 0, "nothing allocated");
    }

    #[test]
    fn a_run_of_one_reads_back_through_its_accessor() {
        let mut stores = ToyStores::new();
        let lists = &mut stores.lists;
        let mark = lists.leaf_scratch.mark();
        lists.leaf_scratch.push(Leaf(7));
        let leaf = lists.leaf_scratch.flush_copy_short(mark, &mut lists.leaves);
        let mark = lists.span_scratch.mark();
        lists.span_scratch.push(span(3));
        let spans = lists.span_scratch.flush_copy(mark, &mut lists.spans);
        let name = stores.text.push("one");
        let lists = &mut stores.lists;
        let mark = lists.name_scratch.mark();
        lists.name_scratch.push(name);
        let names = lists.name_scratch.flush_copy_short(mark, &mut lists.names);
        assert_eq!(leaf.len(), 1);
        let view = stores.view();
        assert_eq!(view.leaves(leaf), [Leaf(7)]);
        assert_eq!(view.spans(spans), [span(3)]);
        assert_eq!(view.text(view.names(names)[0]), "one");
        assert_eq!(stores.lists.leaves.len(), 2, "slot 0 and the one row");
    }

    /// The marks rewind every scratch stack to where it stood, more than once, and leave the
    /// frames opened below them whole.
    #[test]
    fn marks_and_truncate_rewind_every_scratch_stack() {
        let mut stores = ToyStores::new();
        let leaf_frame = stores.lists.leaf_scratch.mark();
        stores.lists.leaf_scratch.push(Leaf(1));
        let branch_frame = stores.lists.branch_scratch.mark();
        let kept = branch(&mut stores, "kept", Run::EMPTY);
        stores.lists.branch_scratch.push(kept);
        stores.lists.token_scratch.push(Token(1));
        stores.lists.span_scratch.push(span(1));
        stores.lists.name_scratch.push(PoolText::EMPTY);

        let mut group = MarkGroup::new();
        let marks: ToyMarks = stores.lists.marks(&mut group);
        group.check();
        for _ in 0..2 {
            stores.lists.leaf_scratch.push(Leaf(2));
            let dropped = branch(&mut stores, "dropped", Run::EMPTY);
            stores.lists.branch_scratch.push(dropped);
            stores.lists.token_scratch.push(Token(2));
            stores.lists.token_scratch.push(Token(3));
            stores.lists.span_scratch.push(span(2));
            stores.lists.name_scratch.push(PoolText::EMPTY);
            let copied = marks;
            stores.lists.truncate(&copied);
            let heights: Vec<_> = stores
                .shapes()
                .into_iter()
                .filter(|shape| shape.kind == FieldKind::Scratch)
                .map(|shape| shape.len)
                .collect();
            assert_eq!(heights, [1, 1, 1, 1, 1]);
        }

        let lists = &mut stores.lists;
        let leaves = lists.leaf_scratch.flush_copy(leaf_frame, &mut lists.leaves);
        let branches = lists
            .branch_scratch
            .flush_copy(branch_frame, &mut lists.branches);
        assert_eq!(stores.view().leaves(leaves), [Leaf(1)]);
        assert_eq!(names(&stores, branches), ["kept"]);
    }

    #[test]
    fn clear_scratch_empties_every_scratch_stack_and_nothing_else() {
        let mut stores = ToyStores::new();
        fill(&mut stores);
        let filled = stores.shapes();
        assert_every_field_written(&filled);
        #[cfg(debug_assertions)]
        assert!(!stores.lists.scratch_is_empty());
        stores.lists.clear_scratch();
        #[cfg(debug_assertions)]
        assert!(stores.lists.scratch_is_empty());
        let expected: Vec<FieldShape> = filled
            .iter()
            .map(|&shape| match shape.kind {
                FieldKind::Scratch => FieldShape { len: 0, ..shape },
                FieldKind::Store | FieldKind::Pool => shape,
            })
            .collect();
        assert_eq!(stores.shapes(), expected);
    }

    /// One scratch stack left open is enough for the check to read the stores as not empty.
    #[test]
    #[cfg(debug_assertions)]
    fn scratch_is_empty_reads_every_scratch_stack() {
        let mut stores = ToyStores::new();
        assert!(stores.lists.scratch_is_empty());
        stores.lists.leaf_scratch.push(Leaf(1));
        assert!(!stores.lists.scratch_is_empty());
        stores.lists.clear_scratch();
        stores.lists.span_scratch.push(span(1));
        assert!(!stores.lists.scratch_is_empty());
        stores.lists.clear_scratch();
        stores.lists.token_scratch.push(Token(1));
        assert!(!stores.lists.scratch_is_empty());
        stores.lists.clear_scratch();
        stores.lists.name_scratch.push(PoolText::EMPTY);
        assert!(!stores.lists.scratch_is_empty());
        stores.lists.clear_scratch();
        let open = branch(&mut stores, "open", Run::EMPTY);
        stores.lists.branch_scratch.push(open);
        assert!(!stores.lists.scratch_is_empty());
    }

    #[test]
    fn reset_rewinds_every_field_and_keeps_capacity() {
        let mut stores = ToyStores::new();
        fill(&mut stores);
        let filled = stores.shapes();
        assert_every_field_written(&filled);
        stores.lists.reset();
        stores.text.reset();
        let expected: Vec<FieldShape> = filled
            .iter()
            .map(|&shape| FieldShape {
                len: shape.rewound_len(),
                ..shape
            })
            .collect();
        assert_eq!(stores.shapes(), expected);

        // slot 0 survives, so the next list mints straight behind it
        let lists = &mut stores.lists;
        let mark = lists.leaf_scratch.mark();
        lists.leaf_scratch.push(Leaf(1));
        lists.leaf_scratch.push(Leaf(2));
        let run = lists.leaf_scratch.flush_copy(mark, &mut lists.leaves);
        assert_eq!(format!("{run:?}"), "Run(1, 2)");
        assert_eq!(stores.view().leaves(run), [Leaf(1), Leaf(2)]);
        assert_eq!(stores.shapes()[1].capacity, filled[1].capacity);
    }
}
