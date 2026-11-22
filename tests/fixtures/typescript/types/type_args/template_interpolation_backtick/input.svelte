<script lang="ts">
	// Type arguments holding a template literal type whose interpolation holds a backtick
	// or a quote — the scan for the closing `>` steps over the whole template, its
	// interpolations included, so the inner backtick does not end it.

	fn<A, `${'`'}`>(c);
	new Fn<A, `${'`'}`>(c);
	fn<A, B<`${'`'}`>>(c);
	fn<{ a: `${'`'}` }>(c);
	fn<[`${'`'}`]>(c);
	fn<(a: `${'`'}`) => void>(c);
	fn<A, `${'`'}`>`t`;
	fn<A, `${`it's`}`>(c);
	const x = fn<A, `${'`'}`>;

	// A later quote that used to pair with the inner one, closing the list for the wrong
	// reason
	fn<A, `${'`'}`>(c);
	const y = '>(1)';

	fn<A, `${'`'}`>(c);
</script>
