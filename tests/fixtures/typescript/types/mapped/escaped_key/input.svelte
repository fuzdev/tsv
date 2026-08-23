<script lang="ts">
	// A unicode escape is an identifier character, so an escaped type-parameter
	// name heads a mapped type exactly as its decoded spelling does. Prettier
	// decodes the escape, so the frozen rows below are where it survives.
	// prettier-ignore
	type A = { [\u004B in U]: V };
	// prettier-ignore
	type B = { [\u004B in U]?: V };
	// prettier-ignore
	type C = { readonly [\u004B in U]: V };
	// prettier-ignore
	type D = { [\u004B in U as \u004E]: V };

	// The `in` is what makes it a mapped type — the same escaped head followed by
	// `:` is an index signature.
	// prettier-ignore
	type E = { [\u004B: string]: V };

	// The braced spelling is the same identifier character, and the `as` clause and
	// the optional marker compose with it.
	// prettier-ignore
	type P = { [\u{4B} in U]: V };
	// prettier-ignore
	type Q = { [\u004B in U as \u004E]?: V };

	// The decoded spellings those normalize to.
	type F = { [K in U]: V };
	type G = { [K in U]?: V };
	type H = { readonly [K in U]: V };
	type I = { [K in U as N]: V };
	type J = { [K: string]: V };
	type R = { [K in U as N]?: V };

	// A head that is not a bare identifier is a computed key rather than a mapped
	// type, however its name is spelled.
	// prettier-ignore
	type L = { [a.\u0062 in c]: 1 };
	type M = { [a.b in c]: 1 };
</script>
