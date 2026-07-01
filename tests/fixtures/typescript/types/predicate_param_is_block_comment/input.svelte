<script lang="ts">
	// A block comment in a type predicate's parameter->`is` gap is preserved inline
	// (`x /* a */ is T`), matching prettier. Covers a plain predicate, an `asserts`
	// predicate, and a `this` predicate. A line comment can't occur here (a newline
	// before `is` is a parse error), so only the block form reaches this gap.
	function f(x): x /* a */ is T {
		return true;
	}

	function g(x): asserts x /* b */ is T {
		x;
	}

	class C {
		m(): this /* c */ is T {
			return true;
		}
	}
</script>
