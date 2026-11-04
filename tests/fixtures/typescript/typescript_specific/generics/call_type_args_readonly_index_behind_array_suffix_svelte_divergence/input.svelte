<script lang="ts">
	// `readonly` over an operand whose postfix run holds an index BEHIND its array suffix.
	// The empty `[]` leaves no member access, so the operand reads only as a type, as tsc
	// reads it; acorn-typescript refuses `readonly` over an indexed access
	fn<readonly (typeof a)[]['length']>(x);
	fn<readonly (A | B)[]['k']>(x);
	fn<readonly 'a'[]['k']>(x);
	fn<readonly {}[][0]>(x);
	fn<readonly `a`[]['k']>(x);
	fn<readonly 1[]['k']>(x);
	fn<readonly (A | B)[]['k'] | C>(x);
	new Cls<readonly (A | B)[]['k']>(x);
	fn<readonly (typeof a)[0][]['k']>(x);
	fn<readonly (typeof a)[]['length']>`t`;
	outer(fn<readonly (typeof a)[]['length'], B>(x));
</script>
