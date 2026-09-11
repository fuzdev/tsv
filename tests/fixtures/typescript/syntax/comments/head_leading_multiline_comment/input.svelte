<script lang="ts">
	// a multi-line comment glued to a statement head's value prints outside the value's own
	// group, so the value keeps its own layout
	if (
		/* c
		 * d */ aaa ? bbb : ccc
	) {
	}
	while (
		/* c
		 * d */ aaa ? bbb : ccc
	) {}
	do {} while (
		/* c
		 * d */ aaa ? bbb : ccc
	);
	for (
		;
		/* c
		 * d */ aaa ? bbb : ccc;
	) {}
	for (const a of /* c
	 * d */ aaa ? bbb : ccc) {
	}

	// the switch discriminant and a case test
	switch (
		/* c
		 * d */ aaa ? bbb : ccc
	) {
		case /* c
		 * d */ aaa ? bbb : ccc:
	}

	// the head broke after a first comment: the run keeps its break
	if (
		/* x */

		/* c
		 * d */ aaa ? bbb : ccc
	) {
	}

	// an import specifier
	import(
		/* c
		 * d */ aaa ? bbb : ccc
	);

	// template interpolations, plain and tagged, with a line comment ahead of the run
	x = `${
		/* c
		 * d */ aaa ? bbb : ccc
	}`;
	fn`${
		// x
		/* c
		 * d */ aaa ? bbb : ccc
	}`;

	// a computed member index, object key and class key
	x[
		/* c
		 * d */ aaa ? bbb : ccc
	];
	x = {
		[/* c
		 * d */ aaa ? bbb : ccc]: 1
	};
	class A {
		[/* c
		 * d */ aaa ? bbb : ccc] = 1;
	}

	// the export assignment value
	export = /* c
	 * d */ aaa ? bbb : ccc;
</script>
