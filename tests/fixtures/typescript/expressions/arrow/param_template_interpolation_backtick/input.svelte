<script lang="ts">
	// Arrow parameters holding a template literal whose interpolation holds a backtick or
	// a quote — the scan for the `)` that closes the parameter list steps over the whole
	// template, its interpolations included, so the inner backtick does not end it.

	// A default
	const a = (x = `${'`'}`) => x;
	const b = async (x = `${'`'}`) => x;
	const c = (x = `${y}${'`'}`) => x;
	const d = (x = `${`)`}`) => x;
	const e = (x = `${`it's`}`) => x;
	fn(async (x = `${'`'}`) => x);

	// A parameter type, a return type, a type parameter constraint
	const f = (x: `${'`'}`) => x;
	const g = (x): `${'`'}` => x;
	const h = <T extends `${'`'}`>(x: T) => x;

	// A conditional's consequent arrow
	const i = cond ? (x = `${'`'}`): T => x : y;
	const j = cond ? (x): `${'`'}` => x : y;

	// A later quote that used to pair with the inner one, closing the list for the wrong
	// reason
	const k = (x = `${'`'}`) => x;
	const l = ') => 1';

	const m = (x = `${'`'}`) => x;
</script>

{(x = `${'`'}`) => x}
