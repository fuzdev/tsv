<script lang="ts">
	// A `//` after a sole member's authored pipe at a LATER intersection member is the
	// member gap's, and takes that gap's own answer: the comment leads the member on its
	// own line under the `&`, exactly as the pipe-less authoring does. The pipe never
	// survives, and the member keeps the continuation's indent
	type A = B & // c1
		C;

	// a middle member, the intersection continuing after it
	type D = B & // c2
		C &
		E;

	// an object member
	type F = B & { a: 1 }; // c3
</script>
