<script>
	async function f() {
		// An await operand's chain takes a continuation indent, like every other nested
		// binary operand - the parens are the printer's and the chain sits inside them
		const a = await (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccc ??
			ddd);

		// The same when the chain's last operand is self-expanding
		const b = await (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			cccccccccccccccccccccccccccccc ?? [1]);

		// 100 chars - stays inline
		const c = await (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? cccccccccccccccccccccccccc ?? ddd);

		// 101 chars - the chain breaks
		const d = await (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
			ccccccccccccccccccccccccccc ??
			ddd);
	}
</script>
