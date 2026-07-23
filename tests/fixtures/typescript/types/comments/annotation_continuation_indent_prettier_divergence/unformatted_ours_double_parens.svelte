<script lang="ts">
	// Variable annotation, simple type: continuation indents one level
	const e: ((// c1
		X)) = null as any;

	// Variable annotation, intersection type: continuation indents one level
	const f: // c2
		A & B = null as any;

	// Class property, simple type: continuation indents one level
	class C {
		p: ((// c3
			X)) = 1;
	}

	// Function parameter, simple type: continuation indents one level
	function fn(
		a: ((// c4
			X))
	) {}

	// Function return type, simple type: continuation indents one level
	function gn(): ((// c5
		X)) {
		return null as any;
	}

	// Interface property, intersection type: continuation indents one level
	interface I {
		m: // c6
			A & B;
	}

	// Arrow-function return type, simple type: continuation indents one level
	const h = (): // c7
		X => null as any;

	// Variable annotation, intersection type, two line comments: each ends its own
	// line and the whole continuation indents one level (none merge or swallow).
	const j: // c8
		// c9
		A & B = null as any;
</script>
