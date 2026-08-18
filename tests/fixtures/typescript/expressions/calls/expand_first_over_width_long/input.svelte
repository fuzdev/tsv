<script lang="ts">
	// The expand-first hug renders the callback's opening brace on the callee's line, so
	// that head line is what print width measures. At 100 it holds and the hug stands in
	// all three argument printers.
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(() => {}, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);
	new Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(() => {}, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);
	oooooooooooooooooooo.mmmmmmmmmmmmmmmmmmm(() => {}, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);

	// At 101 the hug no longer fits and a parameterless callback has nothing of its own
	// to break, so every argument goes to its own line.
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(
		() => {},
		eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	);
	new Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(
		() => {},
		eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	);
	oooooooooooooooooooo.mmmmmmmmmmmmmmmmmmm(
		() => {},
		eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	);

	// A callback whose parameter list CAN break is measured the same way — at 100 the
	// head line holds and nothing breaks.
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(function (aaa, bbb) {}, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);
	new Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(function (aaa, bbb) {}, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);
	oooooooooooooooooooo.mmmmmmmmmmmmmmmmmmm(function (aaa, bbb) {}, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);

	// ...and at 101 the callback's parameters open while the tail stays inline past its
	// body, rather than every argument breaking out.
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(function (
		aaa,
		bbb
	) {}, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);
	new Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(function (
		aaa,
		bbb
	) {}, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);
	oooooooooooooooooooo.mmmmmmmmmmmmmmmmmmm(function (
		aaa,
		bbb
	) {}, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);

	// A callback that DOES break internally is measured on the head line the hug renders
	// it on. At 100 that line holds.
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(() => {
		a();
	}, eeee);

	new Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(() => {
		a();
	}, eeee);

	oooooooooooooooooooooooooooooooooooooooooooo.mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm(() => {
		a();
	}, eeee);

	// At 101 it no longer holds, and the hug gives way to one argument per line even
	// though the callback itself has plenty to break.
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(
		() => {
			a();
		},
		eeee
	);

	new Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(
		() => {
			a();
		},
		eeee
	);

	oooooooooooooooooooooooooooooooooooooooooooo.mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm(
		() => {
			a();
		},
		eeee
	);
</script>
