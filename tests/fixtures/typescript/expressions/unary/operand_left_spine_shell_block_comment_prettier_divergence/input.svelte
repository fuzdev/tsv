<script lang="ts">
	// A block glued inside the stripped shell at the head of a unary operand leads the
	// operand's doc, so the operand takes the comment-holder pair in one pass.
	typeof (/* c */ x[y]);

	typeof (/* c */ x.y);

	typeof (/* c */ x().y());

	typeof (/* c */ x!);

	typeof (/* c */ []<U>);

	// Every unary operator answers the same.
	void (/* c */ x[y]);

	delete (/* c */ x[y]);

	!(/* c */ x[y]);

	-(/* c */ x[y]);

	+(/* c */ x[y]);

	~(/* c */ x[y]);

	// So does a unary nested in a larger expression.
	z = typeof (/* c */ x.y) < T;
</script>
