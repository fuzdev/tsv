<script lang="ts">
	// a redundant paren around a huggable RETURN type is dropped and changes no layout: the
	// sole parameter still groups on its own, so it stays flat and the return type breaks

	// 100 chars - the whole signature fits, so nothing breaks
	function f1(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: A): ({ a: 1 }) {}

	// 101 chars - the param groups (stays flat) and the return type breaks
	function f2(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: A): ({ a: 1 }) {}

	// class method
	class C {
		m(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: A): ({ a: 1 }) {}
	}

	// bodyless overload signature
	function f3(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: A): ({ a: 1 });

	// interface method signature
	interface I {
		m(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: A): ({ a: 1 });
	}

	// a mapped return type is huggable too
	function f4(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: A): ({ [K in T]: U }) {}
</script>
