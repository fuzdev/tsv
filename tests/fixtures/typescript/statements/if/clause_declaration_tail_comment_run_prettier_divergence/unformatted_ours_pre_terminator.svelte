<script lang="ts">
	// a declaration as a clause body: the pre-`;` comment run keeps its lines, at the `if`'s level
	if (a) type X = Y
	// c1
	; // c2
	else fn1();
	if (b) declare function g(): void
	// c3
	;
	else fn2();
	if (c) declare module 'm'
	// c4
	;
	else fn3();
</script>
