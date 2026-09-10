<script lang="ts">
	// A comment the author wrote INSIDE a required paren pair stays inside it.

	// `as` operand
	const a1 = /* c */ (b + c + d) as T;

	// binary operand
	const a2 = /* c */ (b + c + d) * 2;

	// member-call base
	const a3 = /* c */ (b + c + d).toString();

	// non-null operand
	const a4 = /* c */ (b + c + d)!;

	// null control: `typeof` prints its own pair, so prettier keeps the comment inside too
	const a5 = typeof (/* c */ b + c + d);

	// null control: written outside the pair, both formatters keep it there
	const a6 = /* c */ (b + c + d) as T;

	export { a1, a2, a3, a4, a5, a6 };
</script>
