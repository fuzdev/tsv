<script lang="ts">
	// Parenthesized single type argument (parens required for `&`/`|` precedence):
	// the `<` opens type arguments, not a relational operator
	const intersection = callee < (A | B) & C > (firstArg);

	// Parenthesized function type in a union - parens required
	const fnUnion = callee < (() => void) | null > (firstArg);

	// new-expression with a parenthesized first type argument
	const instance = new Foo < (A | B) & C > (firstArg);
</script>
