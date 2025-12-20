<script lang="ts">
	let a = x as T;
	let b = 1 as const;
	let c = x as T as U;
	let d = x satisfies T;
	let e = obj satisfies Record<string, number>;
	let f = (a + b) as T;
	let g = (x * y) satisfies U;
</script>
