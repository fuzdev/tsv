<script lang="ts">
	// An operator token after a would-be `<...>` close that is not an expression
	// start (acorn's `startsExpr = false`) holds the close, so the whole stays an
	// instantiation expression rather than a relational chain. Matches acorn.

	// `in` / `instanceof` — binary keyword operators.
	const a = f<T> in x;
	const b = f<T> instanceof C;

	// Equality operators.
	const d = f<T> == c;
	const e = f<T> === c;
	const g = f<T> != c;
	const h = f<T> !== c;
</script>
