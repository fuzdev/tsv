<script lang="ts">
	async function f1() {
		// `await` argument that breaks on its own: the break after the comment is forced, so
		// the argument hangs one level under the keyword
		const a1 = await /* x */
		(

		fn1({
			b: 1,
			c: 2
		}));

		await /* x */
		(

		{
			aaa: 1
		});

		// `new` callee whose argument list breaks on its own: the whole tail hangs
		new /* x */
		C1({
			aaa: 1
		});
	}
</script>
