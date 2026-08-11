<script lang="ts">
	// A multiline block the author broke after keeps the break - named and anonymous
	// alike, as the heritage and type-param forms already did.
	const a = class A {
		/* x
y */
		p = 1;
	};

	const b = class {
		/* x
y */
		p = 1;
	};

	const c = class A extends B {
		/* x
y */
		p = 1;
	};

	const d = class A<T> {
		/* x
y */
		p = 1;
	};

	// A line comment keeps its place, with `{` flush below.
	const e = class A {
		// c
		p = 1;
	};

	const f = class {
		// c
		p = 1;
	};

	// A single-line block collapses from any authored position.
	const g = class A /* c */ {
		p = 1;
	};

	const h = class /* c */ {
		p = 1;
	};
</script>
