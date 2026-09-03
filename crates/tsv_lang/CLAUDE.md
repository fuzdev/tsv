# tsv_lang

> Language-agnostic foundation crate for `tsv`

All language crates (tsv_ts, tsv_css, tsv_svelte) depend on tsv_lang. It provides the shared primitives for parsing, formatting, and AST manipulation. External dependencies are minimal: `smallvec`, `thiserror`, `unicode-segmentation`, `unicode-width`, plus `serde_json` behind the `json` feature.

## Modules

Each module's visibility (in parens) reflects `pub use`-only modules (private) vs directly-imported modules (`pub mod`, used as `tsv_lang::doc::{...}` etc.).

- `span` (`span.rs`, private) — `Span { start: u32, end: u32 }` — compact source positions
- `location` (`location.rs`, private) — `LocationTracker` (line/column over a `u32` line-start table, fronted by a 1-entry line-range cache that turns the sequential-emission common case into an O(1) range check; a miss searches *forward* from the cached line by galloping when the offset is ahead of it — which the writer's pre-order walk usually makes true — and bisects the whole table only for a genuinely backward lookup. ⚠️ Three layered entry points, and the layering is measured, not stylistic: the cache hit path reads **no memory**, so a "fast path" that indexes `line_starts` instead is *slower* than a hit. `resolve_span` is the form wire emitters want — it resolves a node's `start` and `end` together, searching `end` forward from `start`'s line and deliberately **not** writing the cache, which keeps it parked on the descending line for the first child's `start`), `ByteToCharMap` (byte → UTF-16 code-unit offsets, stored as a dense `byte − utf16` **delta** table narrowed to `u8` unless the source's multibyte characters are dense enough to outgrow it — lookup stays one O(1) load, `pos = byte − delta[byte]`; `identity()` for byte-space passthrough, and for an ASCII-only source, which carries no table at all), and `LocationMapper` (tracker + map bundle the AST-conversion layers thread — with a real map it emits final char-space positions during conversion, fusing out the post-conversion translation walk; with the identity map it is exact byte-space passthrough). Both source walks are word-at-a-time — `next_non_ascii` for the multibyte splits, `next_ecmascript_terminator` for the line starts, which the LF-only rule shares rather than searching `\n` alone: it wants the `\r` positions anyway, to report whether an ECMAScript-rule table over the same source would differ at all (a lone CR / U+2028 / U+2029; CRLF is one ECMAScript break holding one LF, so it never counts). `new_with_map` returns that as its third value, and `tsv_ts::AcornSeed` is what consumes it — the seeding is a fact about acorn, so it lives with the crate that replaces acorn, not here — see [docs/architecture.md §`loc` lines](../../docs/architecture.md#loc-lines-two-classes-one-per-acorn-parse) (a SWAR has-zero mask whose **lowest set lane alone is guaranteed genuine**; read it with `trailing_zeros`, never a popcount or highest-bit scan — the kernels live in `swar`). The `no-locations` emission path skips the line-start scan entirely — it builds a line-data-free tracker via `LocationTracker::new_map_only` (stub `line_starts`, byte→UTF-16 map only) and emits `start`/`end` offsets with no line/column
- `error` (`error.rs`, private) — `ParseError` with context extraction and caret formatting. **`ParseError` is a newtype over a boxed payload (`struct ParseError(Box<ParseErrorKind>)`), so the type is pointer-sized and the enum is private** — a `const` assert pins both the 8-byte size and the `Box` niche that carries `Result<(), ParseError>` down to a bare pointer. The reason is `Result` sizing, not ergonomics: a `Result<T, E>` is sized by `max(T, E)`, and the payload enum is 96 B, so an inline error made every fallible function whose success payload is smaller than that (`()`, `bool`, `usize`, `TokenKind`) return 96 bytes through memory on its hot `Ok` path — and the three parsers are full of those. **The larger effect is code size, not the data path**: an inline error makes every fallible function build a 96-byte struct into its `Result`'s error slot and move it at each `?`, duplicated across ~600 sites — cold error code that nonetheless shares cache lines with hot code and inflates each function's inlining cost. Boxing the payload once, here, measured −7.0% native `.text` and −16.7% on the `parse` WASM bundle, and it reaches `tsv_ts`, `tsv_css` and `tsv_svelte` alike with **no signature anywhere mentioning a `Box`**. ⚠️ Do **not** re-box at a call site (`Result<T, Box<ParseError>>`) — that is a double indirection, and it was measured to buy nothing over the newtype. Because the enum is private, construction goes through the constructor fns (`ParseError::invalid_syntax` / `unexpected_token` / `unexpected_eof` / `invalid_expression` / `file_too_large`, plus `lex_err` for the lexers), which is what keeps `context` uniformly `None` at construction and filled later by `with_context`; `Display` and `Debug` forward to the inner kind, so rendered messages are byte-identical to what the enum produces. ⚠️ **A position is a byte offset into the document the error will be RENDERED against, never into whatever slice produced it** — `with_context` is handed the whole document, so a position in any other coordinate space silently points at an unrelated construct. The parsers satisfy this by construction (each `current_pos` adds its `base_offset`); a **lexer** does not, because it routinely scans a slice (a Svelte `<script>`/`<style>` island, the CSS declaration-value scan, the Svelte parser's post-jump reseek), so each `Lexer` carries the offset of what it scans and applies `ParseError::shift_position` **once**, at the entry point that PRODUCES the error — never in a wrapper delegating to one, which would shift twice and put the position past the end of the source, where the caret vanishes. The relation is gated by `tests/lexer_error_positions.rs`; no fixture type asserts an error position.
- `config` (`config.rs`, private) — `PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts + `EmbedContext` / `LayoutMode` (no runtime config)
- `doc` (`doc/*.rs`, pub) — Document builder — arena-based Prettier-compatible IR
- `comment` (`comment.rs`, private) — Comment type, classification, and O(log n) range lookup
- `acorn_prefix` (`acorn_prefix.rs`, private) — `AcornPrefix` / `AcornPrefixText`: what acorn SAW in the text ahead of one embedded Svelte parse. Svelte hands acorn a differently prepared string at every island, and for four of its readers that string is **manufactured** — the prefix blanked (`replace(/[^\n]/g, ' ')`), or blanked and then capped with a synthetic token (`read_pattern`'s `(pattern = 1)`, `read_type_annotation`'s `_ as `), or blanked only over the non-whitespace (the `{#snippet}` head). Two wire answers read that preparation rather than the document, which is why one value states it: the **line class** an acorn-owned `loc` was counted under (`counts_ecmascript_lines`, the axis `tsv_ts::AcornSeed` seeds from) and the **indentation** `onComment` dedents a multi-line block comment by (`line_start` + `line_indentation`, read by `printing::strip_comment_indentation`). It lives here rather than beside the seed in `tsv_ts` because the dedent — `onComment`'s mirror — already does, and a fact two crates read is this crate's. ⚠️ The two synthetic tokens are not the same shape: `(` is **spliced** between the prefix and the region, where `_ as ` **overwrites** the five document bytes it covers — so it can swallow an author's `\n` (the line then opens further back than the document's does) and it ends the run at a byte the document has no `[ \t]` reading for. `tsv_svelte`'s parser records the preparation per region (`Root::acorn_regions`) and its writer looks it up by position, exactly as it does the seed. No fixture can carry most of this — the pins are `tests/comment_dedent_manufactured_source.rs` (each reader, each with its null control) and the unit tests beside the code; the template readers also ride the `<!-- prettier-ignore -->`-frozen fixture `tests/fixtures/svelte/syntax/comments/head_multiline_comment_dedent`
- `comment_ledger` (`comment_ledger.rs`, pub, **`comment_check` feature**) — the print-once comment ledger (diagnostic)
- `printing` (`printing.rs`, pub) — String literal formatting, same-line detection, visual width. The three line questions (`is_same_line_scan` / `has_newline_between_scan` / `has_blank_line_between_scan`) read the source bytes — a bounded scan past which the document's line-break table answers, and that table is built ON DEMAND: `LineBreaks` takes the document's verdict up front (`line_terminators_are_lf_only`, one loose-needle pass — is every terminator a `\n`?) and fills its arena-parked table (one entry per terminator's LAST byte, `build_line_breaks_into`) only from the `#[cold]` fallbacks; `LineTable` is the `Copy` handle the printers carry (`LineTable::EMPTY` is the canonical reprint's ERASED layout table, a distinct state answered before a byte is read). The `*_fast` forms binary-search a built table and remain the fallback's search and the tests' oracle — no printer calls them. The format path's `<CR>` fold (`normalize_carriage_returns`) returns a `FoldedSource` — the folded text WITH that verdict, taken by the fold's own pass (`classify_line_terminators`, the same loose-needle loop, recording where the first `\r` is and whether a U+2028 / U+2029 is anywhere) — and `LineBreaks::of_folded` builds on it, so a document that folds is walked once: each language crate's `format_folded_in` is the entry point that takes it (the CLI, the three bindings, every `format_str`); `format_in` classifies the source itself
- `source_scan` (`source_scan.rs`, pub) — Trivia-aware source scanning: the `skip_trivia` cursor and its run-level companion `skip_trivia_run` (`skip_trivia` answers "does trivia START here", `skip_trivia_run` "where does the trivia END" — the question a caller sitting *between two tokens* has, since a gap can alternate whitespace, comment, whitespace, comment; it also owns the two obligations each hand-rolled copy of that loop had to remember, that `skip_trivia` must not be called at `end` and that the whitespace step must move by whole characters, and takes the caller's own language whitespace class rather than `char::is_whitespace`), plus the `find_char` / `find_keyword` / `rfind_keyword` delimiter/keyword finders (skipping JS/CSS comments + strings), the regex helpers — `OperandAnchor` + `skip_regex_literal` (the one piece of `/`-disambiguation `skip_trivia` deliberately leaves out, since it needs previous-token context; `OperandAnchor` is the seam that carries it, deriving the regex-vs-division anchor where a `/` asks for it instead of maintaining it on every scanned byte, and the rule it owns — a string or template ends an operand, a comment does not — is stated only there), the **hop-needle contract** every scan that skips bytes instead of asking `skip_trivia` about each one must satisfy (`TRIVIA_OPENERS` + the `const`-evaluable `covers_trivia_openers`, paired in a `const _` beside each needle array so a hop that could step over a string or comment is a compile error; `trivia_hop_needles` builds the array for a scan whose own byte is only known at runtime, and `is_hop_needle` is the one-byte pre-test that pays in front of a hop whose runs are routinely empty — the choice between those rungs is a pure performance one that no gate can check, so the reason is written at each site), and the balanced-brace pair `scan_to_matching_brace` (the expression-context `{…}` matcher — trivia + regex + template aware) / `skip_template_literal` (interpolation-aware template skip, since `skip_trivia`'s opaque quote-to-quote scan mis-pairs backticks across a nested template like `` `${`x`}` ``). The single chokepoint for re-scanning source between AST nodes — used by AST conversion, all three printers, the Svelte parser (which wraps `scan_to_matching_brace` for its `{…}` tags and shares `skip_template_literal` in its regex-unaware binding-pattern scan), and the TS parser's arrow-vs-paren / type-args lookahead
- `escapes` (`escapes.rs`, private) — Escape sequence handling (quote swapping) — used internally by `printing`
- `whitespace` (`whitespace.rs`, private) — `is_js_whitespace`, the ECMAScript `\s` CharSet, shared because two language crates need it and neither can reach the other (`tsv_svelte`'s `is_svelte_ws` is this set; so is the class `parseCss` skips, and `tsv_css` is a *dependency* of `tsv_svelte`). One definition, one exhaustive per-code-point test against Svelte's own hand-written enumeration. ⚠️ Its module doc is the **workspace-wide whitespace index** — every class in tsv, which oracle each answers to, the reads that deliberately answer to none, and the four crates the `char::is_whitespace` / `str::trim` trap has been found in. Read it before adding or changing any whitespace predicate, in any crate: the recurring bug is a site's crate being taken as evidence about which class it wants
- `json_writer` (`json_writer.rs`, private, **`json` feature**) — `JsonWriter`: the byte-buffer + scalar-emitter primitive the three language crates' wire-JSON writers (`ast/convert/write/`) build on. It lives here, not in a language crate, so `tsv_svelte` can compose `tsv_ts` (embedded `{expr}` / `<script>`) and `tsv_css` (embedded `<style>`) emission into **one** shared buffer by passing `&mut JsonWriter` across crate boundaries; each language crate keeps its own node emitters. Escape/format parity is a contract: static structure and tokens are written verbatim (debug-asserted escape-free), dynamic strings and non-integral `f64` delegate to `serde_json` so escaping and ryu formatting are exactly its, and integers are hand-formatted (two-digit-pair, several per node). `string` reaches that delegation through a **SWAR escape prescan** (`needs_escape`, over the `swar` kernels): identifier names, string-literal bodies and comment text are overwhelmingly escape-free, and `serde_json`'s loop pays a 256-entry table lookup plus `split_at`/`split_first` bookkeeping **per byte** just to establish that, so a clean word-at-a-time answer turns the emission into `token`'s quote-blit-quote and anything else falls through to `serde_json` unchanged. The remainder is finished with an **overlapping final word** rather than a byte loop whenever the slice is at least 8 bytes — sound only because the answer is a boolean over the *union* of the bytes tested, so re-testing already-cleared bytes cannot change it (a position-returning scan like `location`'s could not do this), and worth doing because the strings arriving here are short enough that the tail is a large fraction of the whole scan. ⚠️ The prescan's predicate must stay a superset of `serde_json`'s `ESCAPE` non-zero set (`0x00..=0x1F`, `"`, `\` — **not** `DEL`, not any non-ASCII byte), and like the integer emitters **no corpus can grade it**: a mis-scan only surfaces on an input that actually carries the byte, and several of the boundary bytes never appear in real source. The oracle is `string_matches_serde_json`, which grades against `serde_json` itself over a boundary alphabet **and at every offset across the 8-byte stride** — the axis a word-at-a-time scan fails on, and the one a corpus samples arbitrarily. ⚠️ **Integer emission is arithmetic, and no corpus can grade arithmetic** — a wrong digit width writes a wrong offset that still parses as JSON, so the fixture suite, a byte-diff over thousands of files, and every audit gate can all stay green through the bug; the oracles live beside the code and must stay green. Two emission paths share one arithmetic core (`digit_word`): the direct `u32`/`u64`/`usize` appends, and the **staged run** (`stage_begin` / `stage_raw` / `stage_u32` / `stage_usize` / `stage_flush`) that assembles a whole node header in a writer-owned scratch and appends it once — read `stage_begin`'s doc before touching either, it carries the measured rationale — including the one a reader is most likely to get backwards, that a run's **static** fragments are copied twice (scratch, then flush) so the trade is *appends removed* against *static bytes in the run*, **not** run width, and a wide mostly-static burst can be a net loss — and `digit_word`'s for the codegen constraints (register-only digit generation, one magnitude test that yields both the arm and its width, `inline(always)` on the staged emitters) that any rewrite must preserve
- `sizing` (`sizing.rs`, private) — `estimated_json_capacity` / `estimated_ast_arena_capacity` — pre-size heuristics for the wire-JSON output buffer and the parse-time bump arena
- `output` (`output.rs`, private) — `OutputBuffer` — string building with column tracking
- `swar` (`swar.rs`, `pub mod` for one item) — the word-at-a-time byte-search kernels `location`'s line/multibyte scans, `json_writer`'s escape prescan and the language lexers' token-body scans share: `splat` (the broadcast needle), `zero_lanes` (has-zero), `lanes_less_than` (has-less, `json` feature), all `pub(crate)`. One module rather than one copy per caller because their correctness argument is subtle and identical — a SWAR subtract borrows **across** lanes, so a lane's flag is not independently trustworthy. What holds is that **the lowest set lane is always genuine** (a spurious flag needs a genuine one below it, and a genuine lane always flags itself), which gives position callers `trailing_zeros` and boolean callers `mask != 0`. Read the per-kernel guarantee before adding a caller; the lowest-lane property itself is graded by exhaustive tests beside the code. **`next_byte_of` is the one public item** — "index of the first byte that is one of these `N`", the shape every lexer scan over a token body is: `tsv_ts`'s string / template / line- and block-comment runs and `tsv_css`'s comment run all reach it, because spelled as the compare chain it reads as, LLVM emits `2 + 2N` instructions and `N + 2` branches per byte and **does not vectorize** — the caller's resume at a hit (an escape step, a `*` that opens no `*/`) makes the stride data-dependent. ⚠️ It has a **density axis**: the splats are per call and the word is per eight bytes, so break-even is ~3–4 content bytes and a construct that is routinely empty loses. Numbers, and the loose-class-plus-exact-fallback shape a non-byte-set caller uses (`tsv_ts`'s `<LS>` / `<PS>`), are on the function
- `hash` (`hash.rs`, private) — `FxHasher` + the `FxHashMap` / `FxHashSet` / `FxBuildHasher` aliases: a dep-free multiply-xor hasher for the side tables. The printer/writer tables are integer-keyed (the doc arena's `share_map_scratch`, `tsv_svelte`'s root-inline-run set) and reach the hasher only through its integer methods; every table in the experimental `tsv_check` uses it too — address map, symbol tables and flow-label scratch on integer keys, atom interner and merge globals on **name (`str`) keys**, which route through the byte path. Replacing SipHash there is **byte-identical by construction only while every consumer stays order-free** — they use `get`/`insert`/`contains`/`clear` and never let iteration order reach output, and the two `tsv_check` sites that do iterate sort first. A consumer that iterates one of these and emits in that order would make the hasher observable; that is the constraint to preserve, not the hash quality

Each language parser keeps its own single-token lookahead as `peek: Option<Token>` (the lexer's own token POD), with any decoded escape value parked out-of-band — there is no shared lookahead type.

## Doc Builder

The doc builder is the core of the formatting architecture. Language printers build declarative doc trees; the shared renderer decides layout based on print width.

### Key Types

- **`DocArena`** — Contiguous storage for all doc nodes, plus the text pool (the `String` backing `Pooled`/`MultilineText` bodies) and an inline direct-mapped static cache whose slots carry two halves: the amortized-eager widths behind `text()` statics, and the per-document **interned node** — repeated `text(",")` calls within one format return one shared `DocId` instead of allocating per call (sound because statics are position-free at render, nodes are append-only, and no consumer compares `DocId` identity). The hottest statics and the stateless singleton nodes — `empty()`, the four `Line` kinds, `LineSuffixBoundary`, `BreakParent`, `FlushBreak`, the flow-probe sentinel — are a **prelude** seeded at fixed ids ahead of every document (`PRELUDE` in `arena.rs`, re-seeded by `reset()`), so a literal `text(",")` or a `line()` is a constant with no probe at all — which is why a delimiter or operator a builder *receives* travels as a `DocId` built at the caller (a `&'static str` parameter reaches `text()` as a runtime value and pays the prelude's switch, a mispredicting one, at every call); a `Line` node carries no mode or indent (both supplied per visit by the enclosing render command), so every `line()`/`softline()`/`hardline()`/`literalline()` within one document returns one shared node. The arena also parks a per-render output scratch buffer (`take_render_scratch()`/`park_render_scratch()` — the render analog of `pool_writer()`'s parked scratch): the hot per-piece render-and-write seams (TS whole-program/per-expression, CSS per declaration, Svelte per root node) render through the `*_into` entry points into it, one warm buffer per file instead of an alloc/free per call, with a fresh-fallback empty default so nested renders stay correct. The render loop's work buffers pool the same way — each top-level render borrows the arena's command stack + line-suffix buffer (`RefCell`-backed, cleared at borrow; sub-renders keep their own inline `SmallVec` locals) — and the per-file line-break table parks via `take_line_breaks_scratch()`/`park_line_breaks_scratch()` (a `printing::LineBreaks` per `format_in`, filled on demand from the line questions' cold fallbacks), and the multi-line block-comment builders borrow a parked line-offset scratch (`borrow_line_spans_scratch()` — one `printing::next_lf` pass per comment fills each body line's `(start, end)` range, so the classifier and builders iterate slice-cheap with no per-comment line buffer, and no line is materialized as a `str` to fill it). The doc-build side pools too: the wide-list builders assemble their parts into a `DocBuf` drawn from a recursion-safe free-list (`acquire_docbuf`/`release_docbuf`, or the `PooledDocBuf` RAII guard from `pooled_docbuf()`) — a builder pops a cleared buffer (retaining a prior spill's heap capacity) and returns it on scope exit, so the many transient `SmallVec` spills across a document collapse into a handful of long-lived reused buffers; the free-list keeps **only spilled buffers** (a release drops a never-spilled one — nothing to retain, free to re-construct), so every pooled entry carries real heap capacity and a big-need builder can't pop a virgin buffer while capacity sits deeper in the LIFO; retained across `reset()`; byte-identical — allocation only, never output. A parked node-keyed doc-share map (`share_map_scratch()`, an AST-node pointer → built `DocId` table) backs the TS printer's member-chain argument sharing the same way — the consumer clears it at share-scope entry/exit, so only its table capacity persists instead of a fresh `HashMap` resize chain per printer/file. Heuristic capacity: ~2 nodes per source byte (kept above the post-interning ~0.26/byte density because `estimated_children = nodes/2` must still clear the un-shrunk children demand); the text pool pre-sizes at source/8 (measured per-file demand p50 ≈ 0.17× source). `reset()` clears the node/child/text-pool/memo stores while retaining capacity — O(1) on the node store, since `DocNode` carries no drop glue — so a multi-file driver reuses one arena across files (the doc-IR analogue of the binding crates' `Bump::reset()` reuse); the static cache's width halves deliberately survive `reset()` (they key on `'static` string addresses — warming once per arena lifetime) while the interned node halves are invalidated in O(1) by the reset's `format_gen` bump; the printers borrow `&DocArena` and the caller owns the reusable one (`format_in` on each language crate is the borrowed-arena entry point).
- **`DocId`** (`u32`) — Lightweight, `Copy` handle into the arena. No cloning, no recursive Drop.
- **`DocBuf`** (`SmallVec<[DocId; 8]>`) — Shared stack buffer for assembling a node's doc parts before `concat()` / `fill()`. Most nodes have only a handful of parts, so the common case stays off the heap; larger nodes spill. Used by all language printers (the TS chain / binary-operator printers, the Svelte template printer) as the single canonical doc-parts buffer type. Wide-list builders (statement / object / array / parameter / specifier lists) draw a reusable buffer from the arena's `DocBuf` free-list (`pooled_docbuf()`) rather than allocating a fresh `SmallVec` per call, amortizing the per-spill malloc/free churn (see `DocArena` below). Parts produced one at a time — a chain group's node docs, a flattened child list, a first doc ahead of its rest — go through `concat_iter()`, which pulls three parts before any buffer exists (none → `empty()`, one → itself, two → the pair arm, the rest out of line): three quarters of the buffers the printers assembled held one or two parts, and a one-part concat is its part. A site whose part count is decidable at the site (a value and its `;`, `: T`, a bare type reference) pairs or returns directly rather than assembling. And an emitter whose run is usually EMPTY (a trailing comment run, a deferred `;` run) pushes into the caller's buffer and returns what the caller needs — a cursor, a `bool` — rather than a `DocBuf`: two thirds of the printers' `extend`s were of a buffer that had been minted, returned and consumed empty. A returning form survives only where the caller emits later than it asks.
- **`DocNode`** — Node variants: `Text`, `MultilineText` (a `\n`-separated body rendered with per-line context indent — one pool-stored body for an indentable multi-line block comment), `Line`, `Indent`, `Dedent`, `Group`, `IfBreak`, `Concat`, `Fill`, etc. `DocNode` carries no drop glue (`const`-asserted via `needs_drop`): dynamic text lives in the arena text pool, so `reset()`/drop never walk the node store running destructors. Its size is also pinned by a companion `const` assert — **24 B on 64-bit** (the native flagship), **16 B on wasm32** (the shipped WASM bundles); the size is pointer-width dependent (`AlignRoot`'s `usize`, `DocText::Static`'s fat pointer), so the pin is `cfg`-gated per target. The node store is walked linearly at render, so the AoS layout's cache locality is the point (shrinking the node has been refuted repeatedly on this traversal-bound engine); a variant that bloats it is a deliberate decision, not an accident. ⚠️ The 24 B comes from niche-packing into `Text`'s `DocText` payload, and that packing is **charged back at every `match` over a `DocNode`**: `DocText`'s four sub-tags own discriminant values 0..=3, so a kind switch must fold them together before it can index its jump table. The render loop and the fits walk both peel the fold off by probing ahead of the dispatch (the `Text` test *is* the fold), and in the fits walk the same probe also retires the memo round-trip for a leaf text; the same peel is a measured **regression** in `subtree_layout_fill`, whose commonest kind sits above the fold — it pays only where the peeled kind IS the fold's range. See the comments at all three sites and [architecture.md §DocText](../../docs/architecture.md#doctext-static-pooled-sourcespan-verbatimspan).
- **`DocText`** — Four variants: `Static(&'static str)` (punctuation/keywords), `Pooled(PoolSpan)` (dynamic text, stored in the arena text pool), `SourceSpan(Span)` (verbatim source slice — resolved against `source` at print time; zero allocation for unmodified text such as identifier and element/attribute names, comments, template chunks, already-canonical literals (TS numbers/strings, CSS dimensions), and Svelte markup text, with no `DocArena` lifetime), and `VerbatimSpan(Span)` (`SourceSpan` for a **format-ignored frozen slice** — identical in measurement and render, but **opaque to `will_break`**: a frozen slice's embedded newlines are source layout, not a break the enclosing group must honor, matching prettier's `printIgnored` plain-string docs; built only via `verbatim_source_span`, only by the tsv_ts/tsv_svelte ignore emitters). Width policy: `Pooled`, `SourceSpan`/`VerbatimSpan`, and `Static` always precompute their visual width at build (a real width — clamped below the sentinel by the one `clamp_text_width` every producer goes through — or the newline sentinel — fits never borrows the pool, render skips its column byte-scan; `Static`'s precompute is amortized through the arena's static cache, measured once per unique string per arena rather than per node — the same slots that intern `Static` nodes per document), **with no exceptions**: name slices precompute too. Deferring a name's width did not avoid the scan, it moved it into `render_text`'s column advance and the fits path's own on-demand measure — the two hottest functions on the board — and names are ~15% of all doc nodes (`instructions:u` −0.8…−1.2% across corpora for measuring them eagerly, exactly 0.000% on pure CSS, which emits none). With no exception left there is one sentinel and no deferral mechanism: no `NotComputed`, no on-demand measure, and so **no `source` parameter in the fits walk at all** — a width question is answered from the node.
- **`LineKind`** — `Normal` (space in flat, newline in break), `Soft` (nothing in flat), `Hard` (always newline), `Literal` (newline without indent).

### Text width: the corpus cannot grade it — the equivalence test can

`pooled_text_width` (the eager precompute above) is a **search, not a sum**: the width of a plain one-column ASCII slice *is* its byte count, so one `printing::next_width_relevant` pass asks only for the first byte that could make the answer anything else — a `\t`, a `\n`, or a non-ASCII byte — and finding none has finished the measurement. 98.97% of slices never leave that scan. A hit goes to the cold `pooled_text_width_cold`, which resumes the same scan per tab, answers the newline question before the width one, and hands a non-ASCII slice **whole** to the grapheme walk.

**What a contributor must know before touching any of this: no corpus can tell you that you got the arithmetic wrong.** A width only changes the output once it crosses the print width, so an error on a rare byte leaves every formatted file byte-identical. Corrupting the tab arm by a single column was verified to pass the **fixture suite**, an **11,696-file format diff**, and an **11,696-file wire diff** — every external gate in the repo — and to be caught **only** by the exhaustive equivalence test beside the function, which grades it against `contains('\n')` + `visual_width` on every string of length 0–3 over an alphabet covering each arm (including the control chars and the boundary-crossing grapheme clusters), and on a class byte at every alignment of every length up to 40. Keep that test green; it is the only thing that can fail.

Two traps it exists to catch. The scan mirrors `printing::visual_width`'s **ASCII fast path**, where a control character is **one** column — deliberately *not* `printing::ascii_char_width`, which counts it as **zero** and which only the grapheme-walking path uses. And a non-ASCII byte hands the **whole** slice to the grapheme walk, never the scanned remainder, because a grapheme cluster can begin on the ASCII byte *before* it. A third trap is the scan's own class: narrow it by one byte — drop the `\t` — and every tabbed text measures short by one column per tab, which no corpus can see either. The class is spelled once (`printing::is_width_relevant`) and a `const _` block proves the word loop's lane test agrees with it **at compile time**, byte by byte over all 256 — so narrowing the tail is a compile error, not a silent misparse. The equivalence test then walks a class byte across **every** alignment of a slice up to 40 bytes, because the length-0–3 alphabet above never enters an eight-byte word loop at all.

⭐ **The grapheme path's own ASCII runs are a search too, and they take the OTHER class.** `printing::visual_width_mixed` — the arm a slice holding non-ASCII lands on — measures its maximal ASCII runs by finding where each *printable* stretch ends (`next_non_printable_ascii`, class `0x20..=0x7e`) and counting the bytes between hits, because a printable ASCII byte is exactly one column. **Its class is `ascii_char_width`'s, not `is_width_relevant`'s**, and reaching for the scan above is the trap the paragraph before this one names from the other side: that class treats a control as one column, this one gives it zero, so sharing them would over-count every string holding a control — silently, like everything else here. A second `const _` block proves this class twice over all 256 bytes (against the word loop's lane test, and against `ascii_char_width` itself), and because the corpus holds **7** tabs and **no** other control in the half-megabyte of ASCII runs a TypeScript format pass measures, the runtime grader is generated: each special byte at every alignment of a 0–23-byte run, with a non-ASCII tail and again with a non-ASCII lead.

### Builder API Categories

All methods take `&self` (interior mutability via `RefCell`):

- Text — `text()`, `text_pooled(&str)` (dynamic text, copied into the pool), `multiline_text(&str)`, `pool_writer()` (streaming pooled-text assembly: a `PoolTextWriter` owning an arena-parked scratch buffer — no transient `String`, no pool borrow held open, so interleaved arena calls stay correct; consume-on-finish `finish_text()` / `finish_multiline_text()`; implements `fmt::Write`), `source_span()` (verbatim source slice — element / attribute names, comments, template chunks, already-canonical literals; width measured at build) / `source_span_plain()` (the same node for a span the caller has PROVED holds no `\t`, `\n` or byte ≥ `0x80` — its width is its byte length, so nothing is measured and no `source` is taken; reached by the identifier-name seam, three quarters of every width measure a TS format run makes, and by the verbatim-literal seam — a number is ASCII by grammar, a string's quote choice already read its bytes — which is a further 76% of the rest) / `line_comment_source_span()` (verbatim source slice, no allocation) / `verbatim_source_span()` (format-ignored frozen slice — `will_break`-opaque), `empty()`
- Lines — `line()`, `softline()`, `hardline()`, `literalline()`
- Structure — `group()`, `group_break()`, `indent()`, `dedent()`, `align_root()` (absolute tab level — template-literal root reset), `align()` (sub-tab `align(n)` — literal spaces under useTabs, tab-width-independent alignment)
- Conditionals — `if_break()`, `indent_if_break()`, `conditional_group()`, `gated_state()` (a conditional-group state admitted only while its probe *cannot* fit flat one indent level deeper — the caller owes that geometry; see [`DocNode::GatedState`](src/doc/arena.rs))
- Sequences — `concat()`, `concat_iter()` (parts pulled one at a time; no buffer below three), `fill()`, `join()`, `join_doc()`
- Buffer pooling — `pooled_docbuf()` (RAII `PooledDocBuf`, releases on drop) / `acquire_docbuf()` / `release_docbuf()` — reusable `DocBuf` assembly buffers for wide-list builders
- Context — `with_context()`
- Line suffix — `line_suffix()`, `line_suffix_boundary()`, `break_parent()`, `flush_break()` (flush-scoped: forces only the group the deferred run flushes in)
- Convenience — `wrap()`, `parens()`, `brackets()`, `braces()`
- Inspection — `will_break()`, `can_break()`. ⭐ `will_break()` and the `arena_fits` fast path share **one memo and one walk** (`DocArena::subtree_layout_fill`, cell encoding on `LAYOUT_UNKNOWN`): a forced break implies no flat width by induction over the node kinds, so both answers pack into one `u32` per node and the render-time width walk is a cache read of what the build-time break walk already filled. `can_break()` is separate — it asks a different question (*may* it break) and is newline-blind.
- Transforms — `remove_lines()` / `atomize()` — rebuild a subtree with its lines statically flattened (old nodes stay in the arena, unused). **Two operations, not one function with a strength dial**, so pick by which prettier behavior you want: `remove_lines` is prettier's `removeLines` (breakable lines only; hard lines and `MultilineText` survive — it cannot promise one line), while `atomize` emulates a re-render at `printWidth: Infinity` (hard lines deleted, `conditional_group` collapsed to its least-expanded state). Atomizing is only sound where the caller has proved no newline is required — deleting a hard line fuses the content around it. The atomize contract is asserted directly by a width-invariance test: its result must render identically at every width
- Diagnostics — `line_comment_text_pooled()` (tags `//` text for the swallow check)

The `doc::swallow` module is a render-time guard against the
line-comment-swallow bug class (a `//` emitted inline runs to EOL and consumes
the following token). It lives behind the **`swallow_check` cargo feature** (off
by default, like tsv_ts's `convert`), so production builds compile it out
entirely — no `DocArena` side-set, no render hook; `line_comment_text_pooled`
collapses to `text_pooled`. With the feature, `set_swallow_check(true)` arms it
and the renderer (via `SwallowTracker`) records every swallow into a thread-local
sink drained by `take_swallow_reports()`. Output-neutral. `tsv_debug` forwards
the feature as its own opt-in `swallow_check` feature (off by default so its
profiles measure production-shaped render code) and gates the `swallow_audit`
command behind it — build with `--features swallow_check` to drive
`tsv_debug swallow_audit`.

### Rendering Pipeline

```
Language Printer builds DocId tree
        ↓
arena_fits_with_lookahead()  — check if group fits in remaining width
        ↓
arena_print_doc*()           — render doc tree to formatted string
```

**Rendering variants** (the `_resolved_*_into` forms are the production seam):

- `arena_print_doc()` — standard (column 0, no source: for docs with no `SourceSpan`)
- `arena_print_doc_with_indent_resolved_into()` — source-resolved render into a caller-provided buffer (full control)
- `arena_print_doc_with_indent_resolved_preserve_whitespace_into()` — same, preserving last-line whitespace (HTML pre/textarea)

The `_resolved_*_into` forms thread the document `source` (so `DocText::SourceSpan` leaves resolve to their verbatim slice) and render into a caller-provided buffer, reserving `estimated_output_capacity` themselves — the seam behind the arena-parked render scratch the per-piece writers use. `arena_print_doc` passes no source, since its docs contain no `SourceSpan`. (`arena_measure_doc_flat_resolved` renders flat for *measuring* only — never written to output.)

**Below those entry points, the render path threads one `&RenderCtx`.** The mutually-recursive
internals (`render_doc_iterative` → `render_doc_core` → `render_single_doc` /
`render_fill_iterative`, plus the line-suffix flush) each need the same four invariants — the
arena, the `RenderConfig`, the `EmbedContext`, and the document `source` (`Option<&str>`, for
resolving `DocText::SourceSpan` leaves) — so those are bundled into `RenderCtx` and every entry
point constructs one. Each internal function destructures it back
into locals at entry, so the render logic reads unchanged.

⚠️ `RenderCtx` holds **only shared references, deliberately**. The mutable render state —
`output`, `pos`, `should_remeasure`, and the command / line-suffix work buffers — stays as
separate `&mut` parameters, which is why several render functions still carry a
`clippy::too_many_arguments` allow. Bundling those behind a struct pointer takes their address
and sinks them out of registers in the hot loop; the allow is the cheaper price. Don't "finish
the job" by folding them in without an instruction-count gate.

## How Language Crates Use tsv_lang

### Parsing

```rust
// Language parsers use:
use tsv_lang::{ParseError, Result, Span};
use tsv_lang::LocationTracker;  // For wire-JSON emission (root re-export)
use tsv_lang::Comment;          // Collected during parsing (root re-export)
// Lookahead is each parser's own `peek: Option<Token>` over its lexer's token POD.

// Errors enriched with context:
parser.parse().map_err(|e| e.with_context(source))
```

### Formatting

```rust
// Create arena, build doc tree, render:
let arena = DocArena::for_source(source); // sized for source.len()
// The per-document environment (tsv_ts::PrinterInputs); the flags are computed once
// per document — two from the comment list, the line table's verdict by one pass over
// the bytes (`LineBreaks::new`; its table fills only if a line question falls back to
// it) — never per embedded island.
let line_breaks = LineBreaks::new(source, arena.take_line_breaks_scratch());
let inputs = PrinterInputs {
	source,
	comments,
	line_table: line_breaks.table(),
	has_owned_comments,
	has_format_ignore,
};
let mut printer = Printer::with_context(&arena, &inputs, EmbedContext::default(), source.len());
printer.print_program(&program);
let output = printer.into_string();
```

### AST Conversion

```rust
// Internal AST → wire JSON, emitted directly by the writer in one walk:
let (tracker, map) = LocationTracker::new_ecmascript_with_map(source);
let bytes = write_program_json(&program, source, LocationMapper { tracker: &tracker, map: &map }, Schema::Acorn, true);
// The trailing `locations` flag selects the loc-bearing drop-in wire (`true`) or the span-only
// `no-locations` variant (`false`).
// The LocationMapper carries the ByteToCharMap so the writer emits final UTF-16
// positions directly (identity/byte-space passthrough on ASCII). Pass
// Schema::SvelteScript when writing a Svelte non-lang="ts" <script>
// (Svelte's parser omits importKind/exportKind=value and always emits
// `attributes` on import/export declarations).
```

`Schema` is defined in `tsv_ts::ast::convert::Schema`, not in tsv_lang — see [../tsv_ts/CLAUDE.md §Distinctives](../tsv_ts/CLAUDE.md#distinctives).

## Comment Utilities

See [../../docs/comments.md](../../docs/comments.md) for the detached model rationale, the `Comment` struct, the ownership doctrine, and the leading-comment emitter rules; the always-loaded core is [../../CLAUDE.md §Comment Handling](../../CLAUDE.md#comment-handling-detached-model).

### Lookup Functions — three questions, three names

`Comment::owned_by_node` takes a comment out of the *positional* model: the node its token
begins prints it. **Ownership is a fact about who PRINTS a comment, never about whether it
EXISTS** — so the API asks the caller to name which of the three questions it is asking, and
every name states its axis. A miswire then reads as a category error at the call site rather
than as plausible code. See [../../CLAUDE.md §Comment Handling](../../CLAUDE.md#comment-handling-detached-model).

**to emit** — "which comments must *I* print here?" — **skips** owned:

- `comments_to_emit_in_range()` / `has_comments_to_emit_in_range()` / `comments_to_emit_after()`

**on page** — "does any comment occupy the page here?" — **counts** owned. Every layout gate
(break / expand / hug / paren / fast-path / force-multiline):

- `has_comments_on_page_in_range()` / `has_multiline_block_comments_on_page_in_range()`

**in source** — "what comment bytes are physically here?" — **counts** owned. Every cursor
(blank-line scan, offset, `prev_end`):

- `comments_in_source_range()` / `comments_in_source_after()`

Axis-free (provably): `has_line_comments_in_range()` — ownership only ever binds a **block**
comment, so skip ≡ count. If a line comment ever becomes ownable, it must grow an axis.

**The ownership lookup itself** — `owned_leading_comment_at(source, comments, start)` returns the
block comment **owned** by the token beginning at `start`, or `None`. It is not a fourth axis but
the question *behind* the split: it names the comment the to-emit axis skips, so a builder that
**replaces** a token's doc (a format-ignore freeze, a reassembled arrow signature) can print it
instead — otherwise the comment reaches no emitter at all (hazard 1). It lives here rather than
as a twin in each printer because both `tsv_ts` and `tsv_svelte` ask it and the answer is a pure
function of the source bytes plus the comment array; a second copy is exactly the drift the
shared-emitter rule exists to prevent, and it has already cost one recurrence of hazard 1 across
the two crates.

Shared:

- `find_first_comment_from()` — index of the first comment with `span.start >= pos`, and the
  **one physical entry point** all three axes funnel through. A thread-local one-entry hint
  answers the overwhelming majority of asks without searching (the printers walk a document
  in step) and falls back to a `partition_point`; the hint is **verified against the array on
  every read**, so a hint left by another document — or by an array rebuilt at the same
  address — is a miss and never a wrong answer, which is what lets it live unowned with no
  invalidation protocol and no reset
- Every *range* lookup short-circuits a range too narrow to hold a whole comment
  (`Comment::MIN_SPAN_LEN`, guarded at the three parsers' construction sites) without
  probing the array at all — the printers ask about token-sized gaps by the hundred
  thousand, and the chain grouping's member gap is the `.` alone in the overwhelming
  majority of its asks
- `classify_comment()` — Classify as Trailing, LeadingOwnLine, or LeadingInline
- `classify_comment_scan()` — Same, against the document's line table (a bounded scan of the source, the table as the fallback)
- `ClassifiedComments::from_range()` — Batch classify all 4 categories in one pass (emit-keyed)

### Print-Once Ledger (`comment_check` feature)

Nothing in the detached model forces a parsed comment to be *printed* — a gap emitter that
never runs, an owned comment whose node reassembles off the ownership seam, a builder
handed `&[]` for its comment slice each silently lose one. `comment_ledger` is the
structural guard (tsv's `ensureAllCommentsPrinted`): each format entry point registers the
comment list it is about to print (`register_parsed`), each emission records one
(`record_emitted`), each raw source slice that carries comments out verbatim records its
range (`record_verbatim_range`), and `take_comment_ledger` reports every comment whose
emit count isn't exactly one — DROPPED or DOUBLE-PRINTED.

The **doc-based** printers (`tsv_ts`, `tsv_svelte`) don't record at build: they tag the
comment's doc node (`DocArena::tag_comment_doc`) and the *renderer* records the emit when
it reaches that node. A builder may assemble one subtree into two `conditional_group`
candidates of which only one renders, so build-time counting reads as a double-print — and
a comment built only into a *losing* candidate would read as printed while being lost.
`tsv_css`, whose printer writes comments straight to its output buffer, records at the
write itself.

Off by default (like `swallow_check`), so production builds — and default `tsv_debug`
builds, whose profiles must measure production-shaped code — compile out the registration,
the `DocArena` side-set, and the render hook. Output is byte-identical either way.
`tsv_debug` forwards the feature and gates `comment_audit` behind it; `deno task
comments:audit` drives it over `tests/fixtures` and is gated in `deno task check`.

### Directive Recognition

`is_format_ignore_directive()` / `is_format_ignore_range_start()` / `is_format_ignore_range_end()` are the single source of truth for the format-suppression directive set — the tsv-native `format-ignore` family plus prettier's `prettier-ignore` family (drop-in compat). Each operates on trimmed comment text and is called by all three language printers (`tsv_ts`, `tsv_css`, `tsv_svelte`), since the comment types differ across crates. See [docs/directives.md](../../docs/directives.md) and [docs/conformance_prettier_ignore.md §Format-ignore directive](../../docs/conformance_prettier_ignore.md#format-ignore-directive).

## Names are span-identity — no interner

There is **no string interner**. Every name a printer emits — TS/JS identifier
names, Svelte element and attribute names — is recovered from the source slice it
occupies (`source[span]`), never from a symbol table:

- **TS identifier names** (`tsv_ts`'s `IdentName`): the name is the leading
  `raw_len` bytes of the node span, re-sliced at every consumer. The rare
  `\u`-escaped or `u16`-oversized name that can't be recovered from source carries
  its decoded form as an `Option<&'arena str>` escape hatch (the parser's already
  arena-allocated `current_decoded`) — read directly, no round-trip.
- **Svelte element/attribute names** (`tsv_svelte`): `Element::name(source)` =
  `source[name_span]` and `Attribute::name(source)` = `source[name_span]` — every
  name is a verbatim source run, a padded `{ shorthand }` included (its `name_span`
  is the identifier alone, as Svelte's `name_loc` is). No stored name field at all.

The one render-time resolution the doc builder needs is [`DocText::SourceSpan`] →
verbatim source slice (`span.extract(source)`). A printer emitting `SourceSpan` passes
its `&str` source to the resolved render entry points (the `_resolved_*_into` forms),
which thread it through the render path as `Option<&str>` — this is how `source` reaches
render without putting a lifetime on `DocArena` (the span lives in the lifetime-less
arena; the source is supplied transiently at render). There is no resolver trait, no
symbol variant, no deferred symbol resolution, and no `string_interner` dependency.

## Config Types

`PRINT_WIDTH` / `TAB_WIDTH` / `INDENT` consts, `EmbedContext`, and `LayoutMode` are covered in [../../CLAUDE.md §Internal Configuration](../../CLAUDE.md#internal-configuration-rust-library-only). tsv has no runtime configuration.

**Embedding knobs**: `base_indent_offset` and `first_line_offset` are how tsv_svelte tells tsv_ts/tsv_css to format at the right indentation level within a Svelte component. `LayoutMode::Embedded` selects ContinuationIndent style for a binary expression at the embedded expression ROOT only (`build_root_expression_doc`); nested binaries format context-free, keyed by parent position like Prettier's `shouldNotIndent` chain. `root_sequence_indents` is the same question for a comma **sequence** at that root, and it is a separate field rather than another `Embedded` consequence because the two positions have different oracles: prettier width-wraps a `{expr}` tag's sequence flush (so tsv matches), and never width-wraps a block head at all (so tsv's own geometry applies) — set by tsv_svelte's block-head builder alone.

⚠️ **The width knobs act at render, `mode` acts at build.** `base_indent_offset` / `first_line_offset` / `suffix_width` are read by the renderer, so they take effect only on the context passed to an `arena_print_doc_*` call. On the context passed to a `build_*_doc` call they are inert — that doc renders later under its host's own context — and only the three build-time fields — `LayoutMode`, `jsdoc_cast_cannot_hang` and `root_sequence_indents` — survive the trip. Setting a width on a builder's context is the recurring mistake here; check `mode` before believing a comment that attributes a layout to one.
