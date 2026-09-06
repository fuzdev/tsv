<script lang="ts">
	// The declarator's rule (`init_left_side_shell_comment`) at the other seams that share
	// `build_assignment_layout`: an assignment expression's RHS and a class field's
	// initializer. The run inside the value's leftmost shell hangs the value under the operator.
	x = (
		// c
		a as any
	) ? b : c;
	y = (
		// c
		a as any
	).p;
	class K {
		p = (
			// c
			a as any
		).q;
	}
</script>
