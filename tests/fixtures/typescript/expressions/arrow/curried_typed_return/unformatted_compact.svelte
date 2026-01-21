<script lang="ts">
	// With return type on outer arrow - ALWAYS breaks
	const a = (x: T): H => (y) => x + y;

	// Without return type - stays inline
	const b = (x: T) => (y) => x + y;

	// Multiple arrows chained with types
	const c = (x: T): U => (y: V): W => (z) => expr;

	// With block body
	const d = (x: T): H => (y) => { return x + y; };

	// Multiple params with types
	const e = (a: A, b: B): Handler => (c, d) => { return c; };
</script>
