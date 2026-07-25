<script lang="ts">
	// a lone huggable argument with an own-line leading comment expands instead of
	// hugging — the hug has no gap emitter, so the comment would be dropped
		new A(
		// c
			{ a: 1 }
	);

		new A(
		// c
			[a, b]
	);

		new A(
		// c
			function () {
			a();
		}
	);

	// an own-line block comment behaves the same
		new A(
		/* c */
			{ a: 1 }
	);

	// a glued block comment is owned by the argument and rides inside its doc, so the
	// hug survives
	new A(/* c */ { a:   1 });

	new A(/* c */ [a,   b]);
</script>
