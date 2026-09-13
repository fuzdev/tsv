<script lang="ts">
	// A leading run in the shell around a sole-member composite does not change who owns
	// the composite's head gap: the shell's run and the `//` after the dropped operator
	// are ONE run in the enclosing member gap, landing exactly where the operator-less
	// authoring puts them - a glued block, an own-line `//`, a multi-line block

	// a later intersection member
	type Mid = b /* c1 */ & // c2
		a;
	type Mid2 = b & // c3
		// c4
		a;
	type Mid3 = b /* a5
b5 */ & // c6
		a;

	// a later union member
	type Alt =
		| b /* c7 */ // c8
		| a;
	type Alt2 =
		| b // c9
		// c10
		| a;
	type Alt3 =
		| b /* a11
b11 */ // c12
		| a;
</script>
