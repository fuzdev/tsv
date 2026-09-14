<script lang="ts">
	// An operator token after a would-be `<...>` close that is not an expression
	// start (acorn's `startsExpr = false`) holds the close, so the whole stays an
	// instantiation expression rather than a relational chain. Matches acorn.
	// Necessary but not sufficient: `>`, `>>`, `>>>` and `<<` carry no `startsExpr`
	// either, yet acorn holds the close for none of them. The first three no parser
	// admits — the `input_invalid_*` files beside this one; `<<` tsc reads as an
	// instantiation and tsv repairs, so it has no invalid file to sit in.

	// `in` / `instanceof` — binary keyword operators.
	const a = f<T> in x;
	const b = f<T> instanceof C;

	// Equality operators.
	const d = f<T> == c;
	const e = f<T> === c;
	const g = f<T> != c;
	const h = f<T> !== c;
</script>
