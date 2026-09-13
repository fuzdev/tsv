<script lang="ts">
	// A paren pair around an instantiation expression stays when the token after its
	// closing `>` would re-lex the type arguments: `+` and `-` read as unary operators
	// (`fn < T > +1`), a `<`- or `>`-led operator merges with the close (`fn < T >= 1`), and
	// `<<` is rejected after a type argument list by acorn-typescript, the parse oracle.
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
	// an instantiation, or a prefix operator's argument, re-lexes the same way.
	const r = a * fn<T> + 1;
	const s = 1 + fn<T> + 2;
	const t = -fn<T> + 1;
	const u = typeof fn<T> > 1;
	const v = fn<T> /* c */ + 1;

	// Every other operator follows a bare instantiation, so the pair strips.
	const n = fn<T> * 1;
	const o = fn<T> <= 1;
	const p = fn<T> == 1;
	const q = 1 + fn<T>;
</script>
