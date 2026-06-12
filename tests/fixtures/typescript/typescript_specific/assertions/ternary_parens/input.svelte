<script lang="ts">
	// Ternary before as/satisfies: parens required
	let a = (x ? y : z) as T;
	let b = (x ? y : z) satisfies T;

	// Assignment before as/satisfies: parens required
	let c = (x = y) as T;
	let d = (x = y) satisfies T;

	// Ternary in angle-bracket assertion: parens required
	let e = <T>(x ? y : z);

	// Assignment in angle-bracket assertion: parens required
	let f = <T>(x = y);

	// Real-world case from corpus
	const result = resolve((hash ? path + ensure_start(hash, '#') : path) as any);
</script>
