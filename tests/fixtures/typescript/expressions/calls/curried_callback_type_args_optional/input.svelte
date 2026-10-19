<script lang="ts">
	// a curried call's own type arguments and `?.` print, hugged and broken out
	fn()<T>('a', () => {
		fn1();
	});
	fn()?.('a', () => {
		fn1();
	});
	fn(x)?.<T>('a', () => {
		fn1();
	});
	a.b()?.('a', () => {
		fn1();
	});
	x = fn()?.('a', function () {
		fn1();
	});
	fn()<T /* c */>('a', () => {
		fn1();
	});
	// type arguments, the hug at 100 chars
	fn()<T>('ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', () => {
		fn1();
	});
	// type arguments, the hug at 101 chars - the arguments break
	fn()<T>(
		'tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		() => {
			fn1();
		}
	);
	// `?.`, the hug at 100 chars
	fn()?.('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', () => {
		fn1();
	});
	// `?.`, the hug at 101 chars - the arguments break
	fn()?.(
		'ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		() => {
			fn1();
		}
	);
	// `?.` and type arguments, the hug at 100 chars
	fn(x)?.<T>('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', () => {
		fn1();
	});
	// `?.` and type arguments, the hug at 101 chars - the arguments break
	fn(x)?.<T>(
		'ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		() => {
			fn1();
		}
	);
</script>
