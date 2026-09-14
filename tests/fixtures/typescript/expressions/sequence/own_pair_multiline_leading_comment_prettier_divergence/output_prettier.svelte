<script lang="ts">
	// a multi-line comment the author wrote inside a sequence's own pair stays inside it
	export default /* c
	 * d */ (b, c);

	// the same sequence as a later operand of another sequence
	x =
		(a,
		/* c
		 * d */ (b, c));

	// the same pair at every value seam that hoists a leading run — the declarator, the
	// assignment, the class field, the binding default, the enum member, the object
	// property and the arrow body: the pair is the sequence's own, so the comment stays in it
	const v1 =
		/* c
		 * d */ (b, c);
	y =
		/* c
		 * d */ (b, c);
	class A {
		f =
			/* c
			 * d */ (b, c);
	}
	function fn1(
		p = /* c
		 * d */ (b, c)
	) {}
	enum E {
		A = /* c
		 * d */ (b, c)
	}
	const o = {
		p:
			/* c
			 * d */ (b, c)
	};
	const fn2 = () => /* c
	 * d */ (b, c);

	// the single-line kind takes the same place
	const v2 = /* c */ (b, c);
</script>
