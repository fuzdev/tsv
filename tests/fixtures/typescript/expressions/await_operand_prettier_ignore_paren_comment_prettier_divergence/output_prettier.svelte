<script lang="ts">
	async function fn1() {
		// A block comment the author wrote inside a frozen operand's parens stays inside
		// them — the same answer the unfrozen `await` operand gives that gap.
		const aaa =
			await // prettier-ignore
			fn2(  bbb  ) /* c */;

		// A line comment in the same gap opens the pair — inline it would swallow the `)`.
		const ccc =
			await // prettier-ignore
			fn2(  ddd  ); // c

		// An operand whose parens are REQUIRED keeps the comment inside them too.
		const eee = await // prettier-ignore
		(fff  ??  ggg) /* c */;
	}
</script>
