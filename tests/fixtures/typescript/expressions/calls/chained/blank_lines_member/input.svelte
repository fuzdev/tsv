<script lang="ts">
	// Optional chaining - blank line before (chain breaks, preserved)
	const a00000000000000000000000000 = obj000000000000000000000000000000

		?.fn000000000(arg)
		?.chain00(arg);

	// Optional chaining - blank line mid-chain (preserved)
	const b = obj
		.method1(arg)

		?.method2(arg);

	// Optional chaining - blank line mid-chain (long, preserved)
	const c00000000000000000000000000 = obj000000000000000000000000000000
		.fn000000000(arg)

		?.chain00(arg);

	// Mixed member types - property access
	const d = obj.prop1.method1(arg).prop2;

	// Computed member - short chain (fits on one line)
	const e = obj[key1].method1(arg)[key2];

	// Computed member with blank line mid-chain
	const f = obj.method1(arg)[key];

	// All member expression types mixed
	const g = obj.prop

		?.method(arg)

		[key].chain();
</script>
