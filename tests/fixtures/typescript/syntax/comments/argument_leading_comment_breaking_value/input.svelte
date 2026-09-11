<script lang="ts">
	// call argument: the run takes its own line inside the expanded arguments
	fn2(
		/* c2 */
		{
			b: 1,
			c: 2
		}
	);

	// new-call argument: same rule as the call
	const n = new Fn1(
		/* c3 */
		{
			b: 1,
			c: 2
		}
	);

	// arrow body: break after `=>`, the run own-line under it
	const w = () =>
		/* c4 */
		({
			b: 1,
			c: 2
		});

	// an argument that fits collapses each soft separator to a space
	async function f2() {
		const e1 = await /* c5 */ arr1;
	}
	const e2 = () => /* c6 */ arr1;
</script>
