<script lang="ts">
	// A line comment after the left operand, before the operator: tsv keeps it
	// trailing the operand and drops the operator (glued to its right operand) to
	// the next line. Prettier relocates the comment past the operator.

	// arithmetic operator
	const a=1// c
	+2;

	// logical operator
	const b=x// c
	&&y;

	// keyword-binary operator: in
	const c=k// c
	in obj;

	// keyword-binary operator: instanceof
	const d=x// c
	instanceof C;

	// a pair the author glued onto one line keeps that line in the run before the
	// operator, the same rule every comment run reads
	// (syntax/comments/separator_glued_run)
	const f =
		g
		/* c1 *//* c2 */
		// c3
		+ h;

	// a same-line block comment stays inline (not a divergence)
	const e=1/* c */+2;
</script>
