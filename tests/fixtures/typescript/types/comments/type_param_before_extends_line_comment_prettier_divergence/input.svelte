<script lang="ts">
	// A line comment in the name→`extends` gap stays after the name; the
	// `extends A` tail continues one indent level in.
	function fn<
		T // c
			extends A
	>(): void {}

	// A stacked run stays with the name; the tail continues below the run.
	type B<
		T // c1
			// c2
			extends A
	> = T;

	// Multi-param: each comment stays on its own parameter's name.
	type C<
		T // c1
			extends A,
		U // c2
			extends A
	> = [T, U];
</script>
