<script lang="ts">
	// A negative literal type needs no parens outside an array element or indexed-access object
	type A = (-1) | (-2n);
	type B = C[(-1)];
	type D = [(-1)?];
	type E = [...(-1)];
	type F = keyof (-1);
	type G = H<(-1)>;
	let a = b as (-1);
	type M = [a: (-1)];
	type N = (-1) extends O ? 1 : 2;

	// A non-negative literal type needs none as an array element or indexed-access object
	type I = (1)[];
	type J = (1n)[K];
	type L = ('a')[];
</script>
