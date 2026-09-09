<script lang="ts">
	// A hugged sole parameter keeps the object type's authored expansion.

	function fn(a: { aaa: Aaaa }): void {}

	declare function decl(a: { aaa: Aaaa }): void;

	const arrow = (a: { aaa: Aaaa }): void => {};

	class Single {
		method(a: { aaa: Aaaa }): void {}
	}

	interface Multiple {
		method(a: { aaa: Aaaa }): void;
		(a: { aaa: Aaaa }): void;
		new (a: { aaa: Aaaa }): void;
	}

	type A = (a: { aaa: Aaaa }) => void;
	type B = new (a: { aaa: Aaaa }) => void;

	// Two parameters: no hug, so both formatters keep the expansion.
	function two(
		a: {
			aaa: Aaaa;
		},
		b: Bbbb
	): void {}

	// An object PATTERN carries no authored break of its OWN in either formatter.
	function pattern({ aaa }: T): void {}

	// But an object-TYPE annotation on that pattern is inside the divergence, and the break it
	// keeps carries into the pattern beside it — which is what prettier itself does at every
	// position where it keeps the group (add a second parameter and prettier prints this shape).
	function patternAnnotation({ aaa }: { aaa: Aaaa }): void {}
</script>
