<script lang="ts">
	// A type keyword opens type arguments only when the `<`'s first token IS that
	// keyword. `string$` and `stringµ` merely begin with one — `$` and every
	// non-ASCII identifier character continue a name — so each `<` below is the
	// less-than operator and the line is a comparison, not an instantiation.
	const a1 = p < string$ ? q : r > s;
	const a2 = p < stringµ ? q : r > s;
	const a3 = p < string$ - 1 > s;
	const a4 = p < nullµ ? q : r > s;
	const a5 = p < true$ ? q : r > s;

	// The same claim where the token past the would-be closing `>` does not start
	// an expression (a template tag, a parenthesized sequence) — the shapes only
	// the type-argument lookahead's own follow-token filter keeps as comparisons.
	const a6 = p < string$ ? q : r > `t`;
	const a7 = p < stringµ ? q : r > `t`;
	const a8 = p < string$ ? q : r > (t, u);
	const a9 = p < null$ - 1 > `t`;

	// An OPERATOR keyword's dispatch still differs from the identifier arm's (an
	// atom's no longer does — both take the same follow-token filter), so an
	// operator lookalike is where a broken word boundary would bite: matched as
	// `keyof`/`unique`, the `-` would read as a type operand and commit to type
	// arguments, making each line a parse error instead of a comparison.
	const c1 = p < keyof$ - 1 > `t`;
	const c2 = p < uniqueµ - 1 > (t, u);

	// The bare keyword still opens them.
	const b1 = fn<string>();
	const b2 = fn<string | number>();
	const b3 = fn<readonly string[]>();
</script>
