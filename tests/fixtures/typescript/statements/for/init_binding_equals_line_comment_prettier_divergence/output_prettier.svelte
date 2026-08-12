<script lang="ts">
	// A line comment before `=` stays after the binding; `= value` indents one level.
	for (
		let a = 0; // c
		a < 10;
		a++
	) {}

	// Two comments around the init clause stay distinct.
	for (
		let b = 0; // c1 // c2
		b < 10;
		b++
	) {}

	// An annotated binding keeps the comment after the annotation.
	for (
		let d: number = 0; // c
		d < 10;
		d++
	) {}

	// A later declarator's gap answers the same way, at its own level.
	for (
		let e = 0,
			f = 1; // c
		e < f;
		e++
	) {}
</script>
