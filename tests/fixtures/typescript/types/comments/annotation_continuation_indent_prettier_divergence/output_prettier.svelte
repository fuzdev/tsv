<script lang="ts">
	// Variable annotation, simple type: continuation indents one level
	const e: // c1
	X = null as any;

	// Variable annotation, intersection type: continuation indents one level
	const f: // c2
	A & B = null as any;

	// Class property, simple type: continuation indents one level
	class C {
		p: // c3
		X = 1;
	}

	// Function parameter, simple type: continuation indents one level
	function fn(
		a: // c4
		X,
	) {}

	// Function return type, simple type: continuation indents one level
	function gn(): // c5
	X {
		return null as any;
	}
</script>
