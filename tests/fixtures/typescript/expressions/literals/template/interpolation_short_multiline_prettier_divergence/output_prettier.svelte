<script lang="ts">
	// Simple identifier - fits inline but user chose multiline
	const a = `value: ${expr}`;

	// Property access - fits inline but user chose multiline
	const b = `path: ${obj.prop}`;

	// Short function call - fits inline but user chose multiline
	const c = `result: ${fn(x)}`;

	// Array access - fits inline but user chose multiline
	const d = `item: ${arr[0]}`;

	// Short ternary - fits inline but user chose multiline
	const e = `flag: ${a ? b : c}`;

	// Template only contains interpolation
	const f = `${expr}`;
</script>
