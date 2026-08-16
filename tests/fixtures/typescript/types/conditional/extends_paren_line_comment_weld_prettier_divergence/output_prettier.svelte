<script lang="ts">
	// A second comment already trailing the extends-type keeps the run in the shell
	type A = T extends (U1 extends V1 ? W1 : X1) // c1 // c2
		? V
		: W;

	// A run of two inside the shell reaches the same weld from the other side
	type B = T extends // c2 // c1
	U
		? V
		: W;

	// The TRUE branch's own shell run is relocated onto the same line, a third
	// contributor no window over this conditional's own gaps can see
	type C = T extends U // c1 // c2
		? V
		: W;

	// Control: a single comment with nothing else on the destination line still
	// relocates to trail the extends-type
	type D = T extends U // c
		? V
		: W;
</script>
