<script lang="ts">
	// A line comment in the name→`extends` gap stays after the name; the
	// `extends A` tail continues one indent level in.
	function fn<
		T extends // c
			A
	>(): void {}

	// A stacked run stays with the name; the tail continues below the run.
	type B<
		T extends // c1
			// c2
			A
	> = T;

	// Multi-param: each comment stays on its own parameter's name.
	type C<
		T extends // c1
			A,
		U extends // c2
			A
	> = [T, U];
</script>
