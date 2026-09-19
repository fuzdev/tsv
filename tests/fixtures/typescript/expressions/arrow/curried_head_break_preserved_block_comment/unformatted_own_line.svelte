<script lang="ts">
	// A preserved multi-line block leads a chain whose heads force the break from the
	// operator's line, at every assignment seam: its own lines are the break.

	// declarator
	const a =
		/* x
y */ ({}) => () => b;

	// assignment
	c =
		/* x
y */ ({}) => () => b;

	// compound assignment
	c ??=
		/* x
y */ ({}) => () => b;

	class C {
		// class property
		p =
			/* x
y */ ({}) => () => b;
	}

	const obj = {
		// object property
		p:
			/* x
y */ ({}) => () => b
	};
</script>
