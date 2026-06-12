<script lang="ts">
	// Assignment inside arrow function inside call
	const a = b.map((x) => {
		const c = x.d.filter((y) => y.e).map((y) => y.f);
		return c;
	});

	// Assignment in callback with multiple levels
	const n = o.map((p) =>
		p.q.map((r) => {
			const s = r.t.filter((u) => u.v).map((u) => u.w);
			return {x: r.y, z: s};
		}),
	);

	// Assignment inside try block inside callback
	const aa = bb.map((x) => {
		try {
			const cc = x
				.fn()
				.filter((y) => y.dd)
				.map((y) => y.ee);
			return cc;
		} catch {
			return [];
		}
	});
</script>
