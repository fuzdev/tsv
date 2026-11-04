<script lang="ts">
	// `readonly` over a type query whose postfix run holds an index AHEAD of its array
	// suffix: the empty `[]` leaves no member access, so the operand reads only as a type
	fn<readonly (typeof a)['k'][]>(x);
	fn<readonly (typeof a.b)[0][]>(x);
	fn<readonly (typeof a)['k'][][]>(x);
	fn<readonly (typeof a)['k'][]>`t`;
	outer(fn<readonly (typeof a)['k'][], B>(x));
</script>

<p>{fn<readonly (typeof a)['k'][]>(x)}</p>
