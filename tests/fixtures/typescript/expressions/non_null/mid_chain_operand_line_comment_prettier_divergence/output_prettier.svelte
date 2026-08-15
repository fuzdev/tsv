<script lang="ts">
	// A line comment in the operand→`!` gap keeps the operand's parens and its
	// own line where a member access continues the chain past the `!`.
	aaa! // c
		.bbb();

	// The same gap where a call or a computed access continues the chain.
	const b =
		fn! // c
		(1);
	const c =
		aaa! // c
		[0];

	// The operand is itself a call.
	fn()! // c
	.bbb;
</script>
