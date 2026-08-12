<script lang="ts">
	// A line comment before `=` stays after the binding; `= value` indents one level.
	for (
		let a // c
			= 0;
		a < 10;
		a++
	) {}

	// Two comments around the init clause stay distinct.
	for (
		let b // c1
			= 0; // c2
		b < 10;
		b++
	) {}

	// An annotated binding keeps the comment after the annotation.
	for (
		let d: number // c
			= 0;
		d < 10;
		d++
	) {}

	// A later declarator's gap answers the same way, at its own level.
	for (
		let e = 0,
			f // c
				= 1;
		e < f;
		e++
	) {}
</script>
