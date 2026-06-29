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

	// a same-line block comment stays inline (not a divergence)
	const e=1/* c */+2;
</script>
