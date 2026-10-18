<script lang="ts">
	// A type annotation has no break point before the type, so a nullable union type argument
	// breaks inside the `<>` at the 101-char boundary.

	interface I {
		// 100 chars - stays inline (both us and Prettier)
		aaaaaaaaaaaaaaaaaaaaaaaa01: Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>;

		// 101 chars - breaks inside `<>` (Prettier leaves the line over print width)
		aaaaaaaaaaaaaaaaaaaaaaaa02: Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>;
		aaaaaaaaaaaaaaaaaaaaaaaa03: Promise<null | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb>;
		aaaaaaaaaaaaaaaaaaaaaaaa04: Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | void>;

		// a member with a type-argument list that can break keeps the union whole and breaks
		// inside the outer `<>` (Prettier breaks inside the inner `Map<>`)
		aaaaaaaaaaaaaaaaaaaaaaaa11: Promise<Map<
			Kkkkkkkkkk,
			Vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv
		> | null>;
	}

	// a type literal, a class property, and a variable annotation take the same layout
	type T = {
		aaaaaaaaaaaaaaaaaaaaaaaa05: Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>;
	};
	class C {
		aaaaaaaaaaaaaaaaaaaaaaaa06: Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>;
	}
	declare const aaaaaaaaaaaaaaaaaaaaaaaa07: Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>;
	let aaaaaaaaaaaaaaaaaaaaaaaa08: Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>;

	// a return type with no parameters to break, the members in any order
	function aaaaaaaaaaaaaaaaaaaaaaaa09(): Promise<null | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb> {}
	class D {
		aaaaaaaaaaaaaaaaaaaaaaaa10(): Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | void> {}
	}

	// a member with a type-argument list that can break, in a return type
	function aaaaaaaaaaaaaaaaaaaaaaaa12(): Promise<Map<
		Kkkkkkkkkk,
		Vvvvvvvvvvvvvvvvvvvvvvvv
	> | null> {}
</script>
