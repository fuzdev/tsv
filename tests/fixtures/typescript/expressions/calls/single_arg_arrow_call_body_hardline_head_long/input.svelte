<script lang="ts">
	// 100 chars - the head stays on the call line
	fn(({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
		items.map((x) => {
			return x;
		})
	);

	// 101 chars - the argument breaks out
	fn(
		({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
			items.map((x) => {
				return x;
			})
	);

	// new - 100 chars
	new Xy(({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
		items.map((x) => {
			return x;
		})
	);

	// new - 101 chars
	new Xy(
		({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
			items.map((x) => {
				return x;
			})
	);

	// member chain - 100 chars
	obj.a.fn(({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
		items.map((x) => {
			return x;
		})
	);

	// member chain - 101 chars
	obj.a.fn(
		({ a, b: c, d: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee }, i) =>
			items.map((x) => {
				return x;
			})
	);

	// identifier parameters - 100 chars
	obj.a.fn((aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, i) =>
		items.map((x) => {
			return x;
		})
	);

	// identifier parameters - 101 chars
	obj.a.fn(
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, i) =>
			items.map((x) => {
				return x;
			})
	);
</script>
