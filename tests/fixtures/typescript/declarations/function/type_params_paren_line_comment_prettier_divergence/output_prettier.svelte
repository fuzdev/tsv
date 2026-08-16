<script lang="ts">
	// arrow — the comment stays between `>` and `(`, the parameters continue one level in
	const a = <T extends A>( // c1
		p: T
	) => p;

	// function declaration
	function fn1<T>(p: T) { // c2
		return p;
	}

	// function expression
	const b = function <T>(p: T) { // c3
		return p;
	};

	// class method
	class Single {
		m<T>(p: T) { // c4
			return p;
		}
	}

	// two stacked line comments stay distinct and in order
	const c = <T extends A>( // c5
		// c6
		p: T
	) => p;

	// a block ahead of the line comment keeps its place in the run
	const d = <T extends A>( // c7
		/* c8 */
		p: T
	) => p;

	// a block comment does not end its line, so it stays inline
	function fn2<T> /* c9 */(p: T) {
		return p;
	}
</script>
