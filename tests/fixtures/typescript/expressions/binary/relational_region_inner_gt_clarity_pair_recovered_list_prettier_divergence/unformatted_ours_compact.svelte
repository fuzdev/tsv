<script lang="ts">
	// tsc's type-argument parse is error-recovering: a `)` or `]` it is missing is reported and
	// stepped over, so a `>` INSIDE a pair or a bracket the region holds is still the first
	// token its list cannot take, and a `(` past it commits the list — errors and all. The
	// clarity pair behind that `>` is what the compiler would reject, so the operand prints bare.

	// The pair an assertion puts around its comparison.
	fn(a<b,(c>await d) as T);
	fn(a<b,(c>await d) satisfies T);
	fn(a<b,(c>d%e+f) as T);

	// A union's operand pair, a prefix operator's, and an array.
	fn(a<b,c|(d>await e));
	fn(a<b,!(c>await d));
	fn(a<b,[c>await d]);

	// A `<<` is two `<` to that parse, and a `>>` closes both. Behind a `<` of its own the two
	// nest under it, and a `>>>` closes all three.
	fn(a<<b,c>>await d);
	fn(a<<b,c>>d%e+f);
	fn(a<b<<c,d>>>await e);
</script>
