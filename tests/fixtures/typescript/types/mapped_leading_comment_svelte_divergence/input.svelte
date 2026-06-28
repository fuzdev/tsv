<script lang="ts">
	// Block comment on own line (multiline mapped type)
	type A = {
		/* comment */
		[K in keyof T]: V;
	};

	// Line comment on own line (multiline mapped type)
	type B = {
		// comment
		[K in keyof T]: V;
	};

	// Block comment authored inline before `[` (multiline mapped type) - kept inline
	type BInline = {
		/* comment */ [K in keyof T]: V;
	};

	// Block comment inline (single-line mapped type)
	type C = { [/* inline */ K in keyof T]: V };

	// Block comment between : and V (multiline)
	type D = {
		[K in keyof T]: /* before value */ V;
	};

	// Block comment between : and V (single-line)
	type E = { [K in keyof T]: /* before */ V };

	// Block comment after V (multiline)
	type F = {
		[K in keyof T]: V /* after value */;
	};

	// Block comment after V (single-line)
	type G = { [K in keyof T]: V /* after */ };
</script>
