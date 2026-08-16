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
			X
	) {}

	// Function return type, simple type: continuation indents one level
	function gn(): // c5
		X {
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

	// Class-method and object-method return types read the same way as a function
	// declaration's - the continuation indents one level below the method's own head
	class K {
		m(): // c10
			X {
			return null as any;
		}
	}
	const n = {
		m(): // c11
			X {
			return null as any;
		}
	};

	// An arrow argument's return type: the continuation indents one level below the
	// arrow's head wherever that head sits, call argument included
	fn2(
		(
			a
		): // c12
			X => {
			call(a);
		}
	);

	// The `function` spelling of that call argument stays HUGGED — its return type is not
	// part of the hug question the arrow's is — so the head's own line is the callee's, and
	// the continuation indents one level below it
	fn2(x, function (a): // c13
		X {
		call(a);
	});
</script>
