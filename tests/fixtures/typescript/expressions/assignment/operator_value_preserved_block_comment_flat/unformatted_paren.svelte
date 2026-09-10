<script lang="ts">
	// A PRESERVED (non-indentable) multi-line block comment leading the value must not
	// BREAK the value's own group, exactly as its indentable sibling must not
	// (`operator_value_indentable_block_comment_flat`): prettier prints a leading comment
	// outside that group, so a value that fits stays flat below the comment.
	//
	// Only the operator's own line differs by kind. A preserved block never takes the
	// comment-driven hang, so whether the `=` ends its line is the VALUE's question: a
	// binaryish value takes prettier's break-after-operator, a ternary or member chain
	// keeps the comment on the operator's line.

	// binary chain
	const a =
		/* c
d */ (b + c + d);

	// ternary
	const e = /* c
d */ (f ? g : h);

	// member chain
	const i = /* c
d */ j.k().l().m();

	// assignment expression
	let n;
	n =
		/* c
d */ (o + p + q);

	// object property
	const r = {
		s:
			/* c
d */ (t + u + v)
	};

	// class field
	class W {
		x =
			/* c
d */ (y + z + a1);
	}

	// null control: a value with no group of its own
	const b1 = /* c
d */ c1;
</script>
