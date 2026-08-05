<script lang="ts">
	// block-body callback in the last call: the chain stays flat, the member tail hugs the `})`
	const a = obj.aa(x).bb((l) => {
		return l;
	}).prop;

	// a multi-member tail hugs the same way
	const b = obj.aa(x).bb((l) => {
		return l;
	}).prop1.prop2;

	// 3+ calls with complex args force the chain open; the tail still hugs the last call
	const c = obj
		.aa(x)
		.bb(y)
		.cc((l) => {
			return l;
		}).prop;

	// a breaking callback before the last call forces the chain open; the tail hugs the last group
	const d = obj
		.aa((l) => {
			return l;
		})
		.bb(x).prop;

	// an object-body arrow in the last call hugs the same way as a block body
	const e = obj.aa(x).bb((l) => ({
		value: l
	})).prop;
</script>
