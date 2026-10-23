<script lang="ts">
	// A line comment written before a statement's `;` trails after it — also when the
	// author left whitespace after the comment's text (the `;` on the next line)
	let d = 1; // c4
	fn(); // c5
	d = 1; /* c6 */ // c7
	fn();
	// c8
	fn();
	// c9
	// c10
	const e = fn(); // c11
	[a, d] = [d, a];
	function f1() {
		if (a) return a; // c12
		for (;;) {
			if (a) break; // c13
			continue; // c14
		}
		throw a; // c15
	}
	do {} while (a); // c16
	if (a) fn(); // c17
	while (a) fn(); // c18
	for (const g of a) fn(); // c19
	label: fn(); // c20
	switch (a) {
		case 1:
			fn(); // c21
		default:
			fn(); // c22
	}
	{
		fn(); // c23
	}
	debugger; // c24
</script>

<button
	onclick={() => {
		fn(); // c25
	}}
></button>
