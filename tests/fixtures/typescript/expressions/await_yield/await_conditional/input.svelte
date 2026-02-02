<script lang="ts">
	// await with conditional - parens required (await has higher precedence than ?:)
	const a = await (x ? y : z);

	// await with logical in conditional - parens required
	const b = await (x || y ? z : w);

	// await with simple identifier - no parens
	const c = await x;

	// await with call - no parens
	const d = await fn();

	// await with member access - no parens
	const e = await obj.method();
</script>
