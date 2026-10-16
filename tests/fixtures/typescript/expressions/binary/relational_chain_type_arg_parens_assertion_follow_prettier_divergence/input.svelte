<script lang="ts">
	// A `<` after a would-be type-argument close opens a type assertion, so the `<`…`>` is
	// a comparison chain on any line, with the assertion as its right operand — after a
	// name, a member and a `new` callee, and inside an argument list.
	a1 = (f < T) > <U>z;
	a2 = (a.b < T) > <U>z;
	a3 = (new f() < T) > <U>z;
	a4 = (f < T) > <U>z.y;
	a5 = (f < T) > <U>`z`;
	g((f < T) > <U>z);
</script>
