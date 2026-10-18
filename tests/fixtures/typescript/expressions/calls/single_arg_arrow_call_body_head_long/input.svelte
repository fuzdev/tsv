<script lang="ts">
	// 100 chars - the head stays on the call line
	fn(({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
		items.map(fn1)
	);

	// 101 chars - the argument breaks out
	fn(
		({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
			items.map(fn1)
	);

	// new - 100 chars
	new Xy(({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
		items.map(fn1)
	);

	// new - 101 chars
	new Xy(
		({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
			items.map(fn1)
	);

	// member chain - 100 chars
	obj.a.fn(({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
		items.map(fn1)
	);

	// member chain - 101 chars
	obj.a.fn(
		({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
			items.map(fn1)
	);

	// 100 chars - the head stays on the call line though the argument would fit on one line
	fffffffffffffffffffffffffffffffffffffffff(({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
		items.map(fn1)
	);

	// 101 chars - the argument breaks out onto one line
	fffffffffffffffffffffffffffffffffffffffff(
		({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) => items.map(fn1)
	);

	// member chain - 100 chars - the head stays on the call line
	obj.a.fffffffffffffffffffffffffffffffffffffffff(({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
		items.map(fn1)
	);

	// member chain - 101 chars - the argument breaks out onto one line
	obj.a.fffffffffffffffffffffffffffffffffffffffff(
		({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeee }, i) => items.map(fn1)
	);
</script>
