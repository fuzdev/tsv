<script lang="ts">
	// an own-line directive between `)` and a return type's `:` stays own-line and freezes
	// the whole `: type` annotation — function, method, arrow, type-member signatures
	function fn() // prettier-ignore
	: {x:   1} {
		return { x: 1 };
	}

	class A {
		m(
			a: string // c
		) // prettier-ignore
		: {y:   2} {
			return { y: 2 };
		}
	}

	const arrow = () // prettier-ignore
	: {z:   3} => ({ z: 3 });

	type B = {
		m() // prettier-ignore
		: {w:   4};
		() /* prettier-ignore */
		: {v:   5};
	};
</script>
