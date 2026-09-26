<script lang="ts">
	// A paren pair around an instantiation expression stays when the token after its
	// closing `>` unsettles the type arguments, and WHICH parser says so splits four
	// ways: `+` and `-` read as unary operators to both (`fn < T > +1`); a `>`-led `>`,
	// `>>` or `>>>` is ambiguous with a re-scanned `>>` to both; `<` and `>=` only tsc
	// refuses; and `<<` only acorn-typescript, tsv's parse oracle.
	const a = fn<T> + 1;
	const b = fn<T> - 1;
	const c = fn<T> < 1;
	const d = fn<T> > 1;
	const e = fn<T> >= 1;
	const g = fn<T> >> 1;
	const h = fn<T> >>> 1;
	const w = fn<T> << 1;
	const i = obj.fn<T> + 1;

	// A non-null assertion cannot follow a type argument list, so the pair stays here too.
	const j = fn<T>!;
	const k = fn<T>!.prop;
	const m = fn<T>!();

	// The pair wraps whatever operand ENDS on the close: a binary whose right operand is
	// an instantiation, a prefix operator's argument, or an angle-bracket assertion's
	// operand, all re-lex the same way.
	const r = a * fn<T> + 1;
	const s = 1 + fn<T> + 2;
	const t = -fn<T> + 1;
	const u = typeof fn<T> > 1;
	const v = fn<T> /* c */ + 1;
	const x = <T>fn<U> + 1;
	const y = a * <T>fn<U> + 1;

	// An inner assertion that takes its own pair ends the operand on a `)`, so the outer
	// one needs none.
	const z = <T>(<U>fn<V>) + 1;

	// Every other operator follows a bare instantiation, so the pair strips.
	const n = fn<T> * 1;
	const o = fn<T> <= 1;
	const p = fn<T> == 1;
	const q = 1 + fn<T>;
</script>

<!-- The same rule in a template expression, where prettier PARSES the bare spelling: Svelte
	hands template expressions to acorn-typescript, while a `<script lang="ts">` body goes to
	prettier's own tsc-based parse, which rejects `fn<T> < 1` outright. So both halves of the
	rule are oracle-visible on these cells — prettier strips the pair, and tsv puts it back on
	the bare spelling. -->
{fn<T> < 1}
{fn<T> >= 1}
{f<A<B>> >= 1}
