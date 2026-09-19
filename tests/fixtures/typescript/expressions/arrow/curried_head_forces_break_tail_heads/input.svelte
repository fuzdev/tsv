<script>
	// A break-forcing head stacks EVERY head of the chain, the plain ones after it too —
	// prettier's shouldBreakChain is one verdict for the whole chain, not per head
	const a =
		({ x }) =>
		(b) =>
		(c) =>
			test;
	const b =
		({ x }) =>
		(b) =>
		(c) =>
		(d) =>
			test;

	// The trigger anywhere in the chain stacks the heads on both sides of it
	const c =
		(a) =>
		({ x }) =>
		(b) =>
		(c) =>
			test;

	// A block terminal hugs the LAST head only
	const d =
		({ x }) =>
		(b) =>
		(c) => {
			return b(c);
		};
	export const e =
		({ x }) =>
		(b) =>
		(c) => {
			return b(c);
		};

	// Every assignment-like site stacks the same way
	f =
		({ x }) =>
		(b) =>
		(c) =>
			test;
	const g = {
		k:
			({ x }) =>
			(b) =>
			(c) =>
				test
	};
	class A {
		p =
			({ x }) =>
			(b) =>
			(c) =>
				test;
	}

	// A hugging terminal still hugs the last head
	const h =
		({ x }) =>
		(b) =>
		(c) => ({ k: 1 });

	// A chain in a stacked head's default parameter is its own chain, in no chain position:
	// it takes the default shape, not the enclosing chain's stacking
	const i =
		({ x }) =>
		(
			b = ({ y }) =>
				(z) =>
					test
		) =>
		(c) =>
			test;
</script>
