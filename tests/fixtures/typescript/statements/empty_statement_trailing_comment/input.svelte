<script lang="ts">
	// Module scope: trailing comment on a dropped `;` attaches to the previous statement
	a(); // mod

	// Multiple stray `;`: the comment still attaches to the previous statement
	b(); // multi

	// Block body: trailing comment on a dropped `;` (last statement)
	function fn() {
		c();
		d(); // keep
	}

	// Block body: trailing comment on a dropped `;` before another statement
	function gn() {
		e(); // mid
		f();
	}

	// Namespace body: trailing comment on a dropped `;`
	namespace N {
		g(); // ns
	}

	// Switch case consequent: trailing comment on a dropped `;`
	switch (x) {
		case 1:
			h(); // case
	}

	// Trailing block comment on a dropped `;`
	i(); /* block */
</script>
