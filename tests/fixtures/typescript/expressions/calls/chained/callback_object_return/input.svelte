<script lang="ts">
	// short object - stays inline
	const a = items.map((x) => ({a: 1, b: 2})).filter(Boolean);

	// multiline object in chain
	const b = items
		.map((x) => ({
			a: x.a,
			b: x.b,
		}))
		.filter(Boolean);

	// destructuring params
	const c = items
		.map(({a, b}) => ({
			a: a === 'x' ? a : 'y',
			b: [{c: b}],
		}))
		.concat(d);

	// multiple chained methods after callback
	const e = items
		.map((x) => ({
			a: x.a,
			b: x.b,
		}))
		.filter(Boolean)
		.sort();

	// nested object
	const f = items
		.map((x) => ({
			a: {
				b: x.b,
				c: x.c,
			},
		}))
		.filter(Boolean);

	// callback with type annotation
	const g = items
		.map((x): {a: string; b: number} => ({
			a: x.a,
			b: x.b,
		}))
		.filter(Boolean);

	// array return - short stays inline (contrast)
	const h = items.map((x) => [x.a, x.b]).filter(Boolean);

	// block body (contrast)
	const i = items
		.map((x) => {
			return {a: x.a};
		})
		.filter(Boolean);

	// nested arrow returning object (curried)
	const j = items
		.map((x) => (y) => ({
			a: x,
			b: y,
		}))
		.filter(Boolean);

	// async arrow returning object
	const k = items
		.map(async (x) => ({
			a: await x.a,
			b: x.b,
		}))
		.filter(Boolean);

	// triple-nested arrow returning object
	const l = items
		.map((x) => (y) => (z) => ({
			a: x,
			b: y,
			c: z,
		}))
		.filter(Boolean);

	// object with spread
	const m = items
		.map((x) => ({
			...x,
			a: 1,
		}))
		.filter(Boolean);

	// multiline array - also hugs
	const n = items
		.map((x) => [
			x.aaaaaaaaaaaaaaaaaaaaaaaaaa,
			x.bbbbbbbbbbbbbbbbbbbbbbbbbb,
			x.cccccccccccccccccccccccccc,
		])
		.filter(Boolean);

	// long string return - soft wraps (contrast)
	const o = items
		.map(
			(x) => 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		)
		.filter(Boolean);
</script>
