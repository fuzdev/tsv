<script lang="ts">
	// A second comment already trailing the extends-type keeps the run in the shell
	type A = T extends // c1
		(U1 extends V1 ? W1 : X1) // c2
		? V
		: W;

	// A run of two inside the shell reaches the same weld from the other side
	type B = T extends // c1
		// c2
		U
		? V
		: W;

	// The TRUE branch's own shell run stays in the `?` gap, so it never shares the
	// destination line: the extends-type's run relocates alone
	type C = T extends (// c1
		U) ? (// c2
		V) : W;

	// Control: a single comment with nothing else on the destination line still
	// relocates to trail the extends-type
	type D = T extends U // c
		? V
		: W;
</script>
