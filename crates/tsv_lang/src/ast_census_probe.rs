//! Parser allocation-demand counters for `tsv_debug ast_demand_census` (the
//! `_internal_ast_census` feature, off by default and outside the `_internal_audits` umbrella;
//! this module and every call site are removed when it is off).
//!
//! `tsv_debug ast_census` walks the wire, so it sees the survivors of a parse. The stores a
//! parse fills are sized by what it **builds**, and two things a survivor count cannot see
//! are in that number: the nodes a speculative parse built before a rewind, and the nodes
//! the cover grammar and the by-value expression boundary leave behind in the arena when
//! they clone a node out of it. This module records both, per element type:
//!
//! - [`mint`]: every node placed in the arena, one single allocation or the rows of one
//!   flushed list, keyed by the Rust type (`std::any::type_name`).
//! - [`mark`] / [`rewind`]: a mint log with a truncation point. A rewind moves everything
//!   minted since its mark to `abandoned`. A nested rewind's own entries are already gone
//!   when an outer one lands, so nothing is counted twice.
//! - [`dead`]: a site that strands a node already placed. Logged like a mint, so a rewind
//!   over the site undoes the claim (the node it stranded survives the abandoned parse).
//! - The scratch counters behind [`list_delta`]: the running total of open list prefixes per
//!   element type and its peak, which [`ListBuf`](crate::store::ListBuf) and
//!   `ast_census_vec!`'s list wrapper feed. Rows of a list that never flushes (a rewind drops
//!   it, or a parse error unwinds past it) reach no store and are not minted.
//!
//! A row's type name is whatever `type_name` prints: an element type stored by value
//! (`tsv_ts::ast::expressions::Expression`), a ref list's element (`&…Expression`), an
//! `Option` of either. The command turns those names into store rows.

use std::any::type_name;
use std::cell::RefCell;

use crate::FxHashMap;

/// What a [`dead`] call claims about the node it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dead {
    /// The node was cloned out of the arena by value, and the original is now unreachable: the
    /// by-value expression boundary.
    DuplicateRoot,
    /// A new allocation replaced the node: a field changed on a node already in the arena, or
    /// a cover-grammar literal converted to its pattern.
    Superseded,
    /// Duplicated, and both copies stay reachable (a shorthand property's key and value). Not
    /// waste: the parse's own tree holds both. Counted so a store row count reads honestly.
    DuplicateKept,
    /// Cloned out of the arena into a parent that holds it by value: the arena row is
    /// stranded, but the node is alive inside its parent (a class field's initializer, an
    /// enum initializer, an `export default` value). Not waste: a by-value slot that became a
    /// store edge would make this a row.
    MovedInline,
}

/// One element type's counters over an armed run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Row {
    /// `size_of` the element, in bytes.
    pub width: usize,
    /// Every row placed, including those a rewind or a dead claim later removed.
    pub minted: u64,
    /// Rows minted inside a parse a rewind abandoned.
    pub abandoned: u64,
    /// Rows stranded by a by-value clone ([`Dead::DuplicateRoot`]).
    pub duplicate_dead: u64,
    /// Rows stranded by a replacing allocation ([`Dead::Superseded`]).
    pub superseded: u64,
    /// Rows duplicated with both copies reachable ([`Dead::DuplicateKept`]).
    pub duplicate_kept: u64,
    /// Rows stranded by a by-value move into a parent's own slot ([`Dead::MovedInline`]).
    pub moved_inline: u64,
    /// Nodes built by value in a parent's slot, never placed in the arena ([`built_inline`]):
    /// no row here, counted so the slot is visible as demand.
    pub built_inline: u64,
    /// The largest total of open list prefixes of this element type at any moment.
    pub scratch_peak: u64,
}

/// What an armed run counted, keyed by `type_name`.
#[derive(Clone, Debug, Default)]
pub struct Tally {
    pub rows: Vec<(&'static str, Row)>,
}

#[derive(Clone, Copy)]
enum LogKind {
    Mint,
    Dead(Dead),
    /// An earlier [`Dead`] claim of this class taken back ([`reclassify`]).
    UndoDead(Dead),
    BuiltInline,
}

struct LogEntry {
    index: u32,
    rows: u32,
    kind: LogKind,
}

#[derive(Default)]
struct State {
    /// `type_name` to [`Self::names`] index. By the string, not its address: each crate's
    /// monomorphization carries its own copy, and a mint in one crate and a claim in another
    /// must land on one row.
    by_name: FxHashMap<&'static str, u32>,
    names: Vec<&'static str>,
    rows: Vec<Row>,
    /// Open-list total per name index, the value [`Row::scratch_peak`] tracks.
    scratch_now: Vec<u64>,
    log: Vec<LogEntry>,
}

impl State {
    fn index_of<T>(&mut self) -> usize {
        let name = type_name::<T>();
        let next = self.names.len() as u32;
        let index = *self.by_name.entry(name).or_insert(next);
        if index == next {
            self.names.push(name);
            self.rows.push(Row {
                width: size_of::<T>(),
                ..Row::default()
            });
            self.scratch_now.push(0);
        }
        index as usize
    }
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state(f: impl FnOnce(&mut State)) {
    STATE.with(|state| {
        if let Some(state) = state.borrow_mut().as_mut() {
            f(state);
        }
    });
}

/// Start counting on this thread, from zero.
pub fn arm() {
    STATE.with(|state| *state.borrow_mut() = Some(State::default()));
}

/// Stop counting on this thread and read what was counted since [`arm`].
pub fn disarm() -> Tally {
    let Some(mut state) = STATE.with(|state| state.borrow_mut().take()) else {
        return Tally::default();
    };
    for entry in std::mem::take(&mut state.log) {
        let row = &mut state.rows[entry.index as usize];
        let rows = u64::from(entry.rows);
        match entry.kind {
            LogKind::Mint => row.minted += rows,
            LogKind::Dead(class) => {
                let field = dead_field(row, class);
                *field = field.wrapping_add(rows);
            }
            // wraps below zero only until the claim it takes back, which is logged later than
            // the undo at a site that reclassifies before the clone it names; the log is
            // applied whole, so the sum is exact
            LogKind::UndoDead(class) => {
                let field = dead_field(row, class);
                *field = field.wrapping_sub(rows);
            }
            LogKind::BuiltInline => row.built_inline += rows,
        }
    }
    Tally {
        rows: state.names.into_iter().zip(state.rows).collect(),
    }
}

fn dead_field(row: &mut Row, class: Dead) -> &mut u64 {
    match class {
        Dead::DuplicateRoot => &mut row.duplicate_dead,
        Dead::Superseded => &mut row.superseded,
        Dead::DuplicateKept => &mut row.duplicate_kept,
        Dead::MovedInline => &mut row.moved_inline,
    }
}

/// Record `rows` nodes of `T` placed in the arena: one single allocation (`rows == 1`) or
/// one flushed list.
pub fn mint<T>(rows: usize) {
    with_state(|state| {
        let index = state.index_of::<T>() as u32;
        state.log.push(LogEntry {
            index,
            rows: rows as u32,
            kind: LogKind::Mint,
        });
    });
}

/// Record that `rows` nodes of `T` already placed are stranded ([`Dead`] says how).
pub fn dead<T>(class: Dead, rows: usize) {
    with_state(|state| {
        let index = state.index_of::<T>() as u32;
        state.log.push(LogEntry {
            index,
            rows: rows as u32,
            kind: LogKind::Dead(class),
        });
    });
}

/// Record `rows` nodes of `T` built by value in a parent's slot and never placed in the arena.
pub fn built_inline<T>(rows: usize) {
    with_state(|state| {
        let index = state.index_of::<T>() as u32;
        state.log.push(LogEntry {
            index,
            rows: rows as u32,
            kind: LogKind::BuiltInline,
        });
    });
}

/// Move a [`dead`] claim from class `from` to class `to`, for a caller that knows better
/// than the clone site what became of the value (a by-value boundary whose result is
/// allocated again rather than held by value).
pub fn reclassify<T>(from: Dead, to: Dead, rows: usize) {
    with_state(|state| {
        let index = state.index_of::<T>() as u32;
        let rows = rows as u32;
        state.log.push(LogEntry {
            index,
            rows,
            kind: LogKind::UndoDead(from),
        });
        state.log.push(LogEntry {
            index,
            rows,
            kind: LogKind::Dead(to),
        });
    });
}

/// A point in the log for [`rewind`] to return to.
pub fn mark() -> u32 {
    let mut len = 0;
    with_state(|state| len = state.log.len() as u32);
    len
}

/// Abandon everything logged since `mark`: mints become [`Row::abandoned`], dead claims
/// vanish (the parse that made them never happened).
pub fn rewind(mark: u32) {
    with_state(|state| {
        for entry in state.log.drain(mark as usize..) {
            if matches!(entry.kind, LogKind::Mint) {
                let row = &mut state.rows[entry.index as usize];
                row.minted += u64::from(entry.rows);
                row.abandoned += u64::from(entry.rows);
            }
        }
    });
}

/// Abandon the log entries in `start..end`: every mint in the span becomes
/// [`Row::abandoned`], its claims vanish. For a parse thrown away after a later one ran
/// (the marks of the later parse, all above `end`, are finished with by then).
pub fn abandon_range(start: u32, end: u32) {
    with_state(|state| {
        for entry in state.log.drain(start as usize..end as usize) {
            if matches!(entry.kind, LogKind::Mint) {
                let row = &mut state.rows[entry.index as usize];
                row.minted += u64::from(entry.rows);
                row.abandoned += u64::from(entry.rows);
            }
        }
    });
}

/// Abandons, on drop, everything minted since it was built: for a function whose whole
/// parse is discarded by contract, on every return path.
pub struct RewindOnDrop(u32);

impl RewindOnDrop {
    pub fn new() -> Self {
        Self(mark())
    }
}

impl Default for RewindOnDrop {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for RewindOnDrop {
    fn drop(&mut self) {
        rewind(self.0);
    }
}

/// Move the open-list total of `T` by `delta` (positive for a push, negative for a flush or
/// a drop) and track its peak.
pub fn list_delta<T>(delta: isize) {
    with_state(|state| {
        let index = state.index_of::<T>();
        let now = state.scratch_now[index] as i64 + delta as i64;
        debug_assert!(now >= 0, "a list's tracked length went negative");
        state.scratch_now[index] = now.max(0).cast_unsigned();
        let row = &mut state.rows[index];
        row.scratch_peak = row.scratch_peak.max(state.scratch_now[index]);
    });
}

/// The parser's list wrapper: `BumpVec<'bump, T>` over `bumpalo::collections::Vec`, feeding
/// [`mint`] at flush and [`list_delta`] at every length change. Expanded in each parser
/// crate behind the feature (`tsv_lang` has no `bumpalo` of its own, and a dependency for
/// the sake of a diagnostic is not worth having).
///
/// Lengths are reconciled at each observation point, so an edit through `DerefMut`
/// (`truncate` beyond the override, `pop`, `clear`) moves the total at the next one.
#[doc(hidden)]
#[macro_export]
macro_rules! ast_census_vec {
    () => {
        pub(crate) struct BumpVec<'bump, T> {
            inner: ::core::option::Option<::bumpalo::collections::Vec<'bump, T>>,
            tracked: usize,
        }

        impl<'bump, T> BumpVec<'bump, T> {
            pub(crate) fn new_in(bump: &'bump ::bumpalo::Bump) -> Self {
                Self {
                    inner: Some(::bumpalo::collections::Vec::new_in(bump)),
                    tracked: 0,
                }
            }

            pub(crate) fn with_capacity_in(capacity: usize, bump: &'bump ::bumpalo::Bump) -> Self {
                Self {
                    inner: Some(::bumpalo::collections::Vec::with_capacity_in(
                        capacity, bump,
                    )),
                    tracked: 0,
                }
            }

            fn vec(&self) -> &::bumpalo::collections::Vec<'bump, T> {
                match &self.inner {
                    Some(vec) => vec,
                    None => unreachable!("only `into_bump_slice` empties it, consuming self"),
                }
            }

            fn vec_mut(&mut self) -> &mut ::bumpalo::collections::Vec<'bump, T> {
                match &mut self.inner {
                    Some(vec) => vec,
                    None => unreachable!("only `into_bump_slice` empties it, consuming self"),
                }
            }

            /// Move the open-list total to this list's actual length.
            fn reconcile(&mut self) {
                let actual = self.vec().len();
                if actual != self.tracked {
                    $crate::ast_census_probe::list_delta::<T>(
                        actual as isize - self.tracked as isize,
                    );
                    self.tracked = actual;
                }
            }

            pub(crate) fn push(&mut self, value: T) {
                self.vec_mut().push(value);
                self.reconcile();
            }

            pub(crate) fn truncate(&mut self, len: usize) {
                self.vec_mut().truncate(len);
                self.reconcile();
            }

            pub(crate) fn into_bump_slice(mut self) -> &'bump [T] {
                self.reconcile();
                let rows = self.tracked;
                if rows > 0 {
                    $crate::ast_census_probe::mint::<T>(rows);
                }
                $crate::ast_census_probe::list_delta::<T>(-(rows as isize));
                self.tracked = 0;
                let vec = self.inner.take();
                match vec {
                    Some(vec) => vec.into_bump_slice(),
                    None => unreachable!("consuming self, so `inner` is still set"),
                }
            }
        }

        impl<'bump, T> IntoIterator for BumpVec<'bump, T> {
            type Item = T;
            type IntoIter = BumpVecIntoIter<'bump, T>;
            fn into_iter(mut self) -> Self::IntoIter {
                self.reconcile();
                let remaining = self.tracked;
                self.tracked = 0;
                let vec = self.inner.take();
                match vec {
                    Some(vec) => BumpVecIntoIter {
                        inner: vec.into_iter(),
                        remaining,
                    },
                    None => unreachable!("consuming self, so `inner` is still set"),
                }
            }
        }

        /// A consumed list: its rows leave the open-list total as they are taken.
        pub(crate) struct BumpVecIntoIter<'bump, T> {
            inner: <::bumpalo::collections::Vec<'bump, T> as IntoIterator>::IntoIter,
            remaining: usize,
        }

        impl<'bump, T> Iterator for BumpVecIntoIter<'bump, T> {
            type Item = T;
            fn next(&mut self) -> Option<T> {
                let item = self.inner.next();
                if item.is_some() {
                    self.remaining -= 1;
                    $crate::ast_census_probe::list_delta::<T>(-1);
                }
                item
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.inner.size_hint()
            }
        }

        impl<'bump, T> Drop for BumpVecIntoIter<'bump, T> {
            fn drop(&mut self) {
                $crate::ast_census_probe::list_delta::<T>(-(self.remaining as isize));
            }
        }

        impl<'bump, T> ::core::ops::Deref for BumpVec<'bump, T> {
            type Target = ::bumpalo::collections::Vec<'bump, T>;
            fn deref(&self) -> &Self::Target {
                self.vec()
            }
        }

        impl<'bump, T> ::core::ops::DerefMut for BumpVec<'bump, T> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                self.vec_mut()
            }
        }

        impl<'bump, T> Drop for BumpVec<'bump, T> {
            fn drop(&mut self) {
                if self.inner.is_some() {
                    self.reconcile();
                    $crate::ast_census_probe::list_delta::<T>(-(self.tracked as isize));
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(tally: &Tally, name: &str) -> Row {
        tally
            .rows
            .iter()
            .find(|(n, _)| n.ends_with(name))
            .map_or_else(|| panic!("no row for {name}"), |(_, row)| *row)
    }

    /// A rewind moves what it abandons out of the live share and takes the dead claims made
    /// inside it with it.
    #[test]
    fn a_rewind_abandons_mints_and_erases_dead_claims() {
        arm();
        mint::<u8>(2);
        let outer = mark();
        mint::<u8>(1);
        dead::<u8>(Dead::DuplicateRoot, 1);
        let inner = mark();
        mint::<u16>(4);
        rewind(inner);
        rewind(outer);
        mint::<u8>(1);
        let tally = disarm();
        let bytes = row(&tally, "u8");
        assert_eq!((bytes.minted, bytes.abandoned), (4, 1));
        assert_eq!(bytes.duplicate_dead, 0);
        let words = row(&tally, "u16");
        assert_eq!((words.minted, words.abandoned), (4, 4));
    }

    #[test]
    fn dead_claims_land_in_their_own_counter() {
        arm();
        mint::<u8>(3);
        dead::<u8>(Dead::DuplicateRoot, 1);
        dead::<u8>(Dead::Superseded, 1);
        dead::<u8>(Dead::DuplicateKept, 1);
        let bytes = row(&disarm(), "u8");
        assert_eq!(
            (bytes.duplicate_dead, bytes.superseded, bytes.duplicate_kept),
            (1, 1, 1)
        );
    }

    /// Two open lists of one type: the peak is the sum of their prefixes, not the longest.
    #[test]
    fn the_scratch_peak_is_the_sum_of_open_prefixes() {
        arm();
        list_delta::<u8>(3);
        list_delta::<u8>(2);
        list_delta::<u8>(-2);
        list_delta::<u8>(1);
        let bytes = row(&disarm(), "u8");
        assert_eq!(bytes.scratch_peak, 5);
    }

    #[test]
    fn an_unarmed_thread_records_nothing() {
        mint::<u8>(9);
        assert!(disarm().rows.is_empty());
    }
}
