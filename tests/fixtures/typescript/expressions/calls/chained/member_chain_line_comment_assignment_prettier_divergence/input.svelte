<script lang="ts">
	// The member-only-chain interior-line-comment divergence
	// (member_only_non_null_line_comment) in assignment-expression and
	// object-property position — the layout path that keeps the chain on the
	// operator line rather than breaking after the operator.

	// Assignment expression, non-null member.
	x = foo // c
		.bar!;

	// Assignment expression, optional member.
	y = foo // c
		?.bar!;

	// Object property value.
	const o = {
		k: foo // c
			.bar!
	};
</script>
