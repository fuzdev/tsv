<script lang="ts">
	// FALSE position: nothing in the conditional follows the branch, so the shell is
	// retained and the comment keeps the line it was written on
	type A = T extends U
		? Z
		: (
			V extends W ? X : Y // c
		);

	// TRUE position: the arm's own `:` still follows, so the shell strips and the
	// comment trails the branch it was written on
	type B = T extends U
		? V extends W
			? X
			: Y // c
		: Z;

	// A false-position shell nested inside a TRUE branch still has the outer `:` to
	// flush against, so it strips too
	type C = A1 extends B1
		? C1 extends D1
			? E1
			: F1 extends G1
				? H1
				: I1 // c
		: J1;

	// Control: with no comment the shell strips at every position
	type D = T extends U ? Z : V extends W ? X : Y;
</script>
