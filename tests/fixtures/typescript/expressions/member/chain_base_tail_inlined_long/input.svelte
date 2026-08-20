<script lang="ts">
	// The positions where `shouldInline` takes that break point back. Both name the same
	// two parents - an assignment value and a declarator initializer - and ask what the
	// lookup's object is: a call WITH ARGUMENTS, or a chain long enough to print as one.

	// a declarator initializer, object = a call with arguments: the width sheds into the args
	const aa1 =
		aaaa.bbbb(
			cccc
		).dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// an assignment value - the same position
	eeeeeeeeee =
		aaaa.bbbb(
			cccc
		).ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff;

	// object = a chain that prints as a member chain: the chain expands, the lookup glues
	const aa2 = aaaa
		.bbbb()
		.cccc().gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg;

	// NOT the clause: the call has no arguments and the chain is too short to print as one
	const aa3 =
		aaaa.bbbb()
			.hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh;
</script>
