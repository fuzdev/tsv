<script lang="ts">
	// a computed member key is a single assignment expression, not a sequence or spread
	interface I {
		[a + b]: number;
		[a ? b : c]: string;
		[(x, y)]: boolean;
	}

	type T = {
		[a = 0]: A;
	};
</script>
