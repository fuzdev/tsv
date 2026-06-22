<script lang="ts">
	// Source is multi-line - stays multi-line (even though it fits on one line)
	type A = {
		[K in keyof U]: V;
	};

	// Source is one-line - stays one-line
	type B = { [K in keyof U]: V };

	// Multi-line with modifiers
	type C = {
		readonly [K in keyof U]?: V;
	};

	// Multi-line with as clause
	type D = {
		[K in keyof U as K]: V;
	};
</script>
