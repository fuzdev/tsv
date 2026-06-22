<script lang="ts">
	// Short object type argument - fits inline
	const a: Array<{ a: string; b: number }> = [];

	// Object type argument that wraps - we should hug the opening brace
	const b: Array<{
		a: string;
		b: Array<{ a: string; b: string }>;
	}> = [];

	// Nested multiline - multiple levels
	const c: Map<
		string,
		{
			a: Array<{ a: string }>;
			b: number;
		}
	> = new Map();
</script>
