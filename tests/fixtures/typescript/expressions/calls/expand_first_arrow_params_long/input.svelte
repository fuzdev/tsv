<script lang="ts">
	// The expand-first hug prints the arrow's parameters flat on the callee's line. At 100
	// that head line holds and the hug stands in all three argument printers.
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa((acc: string, cur: string) => {
		return 1;
	}, '');
	new Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa((acc: string, cur: string) => {
		return 1;
	}, '');
	ooooooooooooooooooooooooooooooo.mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm((acc: string, cur: string) => {
		return 1;
	}, '');

	// At 101 the parameters do not break inside the hug; every argument goes to its own
	// line with the parameters kept on one.
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(
		(acc: string, cur: string) => {
			return 1;
		},
		''
	);
	new Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(
		(acc: string, cur: string) => {
			return 1;
		},
		''
	);
	ooooooooooooooooooooooooooooooo.mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm(
		(acc: string, cur: string) => {
			return 1;
		},
		''
	);
</script>
