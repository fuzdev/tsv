<script lang="ts">
	// `async` as the operand of a cast keeps its pair at every position
	(async) as T;
	(async) satisfies T;
	a = (async) as T;
	fn((async) satisfies T);
	const b = () => (async) as T;

	// a cast chain wraps only the innermost operand
	(async) as T as U;

	// the bare word is fine where no word follows it on its line
	async.b as T;
	async in b;
</script>
