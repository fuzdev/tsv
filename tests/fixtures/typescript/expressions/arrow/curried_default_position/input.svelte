<script lang="ts">
	// A curried chain outside an assignment, a call argument or a binaryish operand takes
	// prettier's default shape: every head past the first indents one level under it, and
	// the terminal body one level under the heads

	// A break-forcing head stacks the heads under the first; a block terminal hugs the last
	// head, its content one level under the heads
	export default ({ x }) =>
		(b) =>
		(c) => {
			return b(c);
		};

	// A short plain chain stays inline
	function fn0() {
		return (a) => (b) => (c) => test;
	}

	// Two heads and three
	function fn1() {
		return ({ x }) =>
			(b) =>
				test;
	}
	function fn2() {
		return ({ x }) =>
			(b) =>
			(c) =>
				test;
	}

	// The trigger on a later head stacks them all the same
	function fn3() {
		return (a) =>
			({ x }) =>
			(b) =>
				test;
	}

	// A hugging terminal hugs the last head
	function fn4() {
		return ({ x }) =>
			(b) => ({ k: 1 });
	}

	// Array element, ternary branch, default parameter, yield, throw
	const arr = [
		({ x }) =>
			(b) =>
				test,
		1
	];
	const t = cond
		? ({ x }) =>
				(b) =>
					test
		: ({ y }) =>
				(c) =>
					test;
	function fn5(
		p = ({ x }) =>
			(b) =>
				test
	) {}
	function* fn6() {
		yield ({ x }) =>
			(b) =>
				test;
	}
	function fn7() {
		throw ({ x }) =>
			(b) =>
				test;
	}

	// Spread, template literal, unary and await operands
	async function fn8() {
		const obj = {
			...({ x }) =>
				(b) =>
					test
		};
		const str = `${({ x }) =>
			(b) =>
				test}`;
		void (({ x }) =>
			(b) =>
				test);
		await (({ x }) =>
			(b) =>
				test);
	}

	// A cast around the chain keeps the chain's shape inside its parens
	const u = (({ x }) =>
		(b) =>
		(c) =>
			test) satisfies T;
	const v = (({ x }) =>
		(b) =>
		(c) =>
			test) as T;
</script>
