<script lang="ts">
	// spread/rest is grammatical only in array/object/call/new argument lists and in
	// rest parameters/patterns — never in an arbitrary expression position
	const arr = [...a];
	const obj = { ...b };
	fn(...arr);
	new C(...arr);
	const rest = (...args) => args;
	const [first, ...others] = arr;
	const { c, ...others2 } = obj;
</script>
