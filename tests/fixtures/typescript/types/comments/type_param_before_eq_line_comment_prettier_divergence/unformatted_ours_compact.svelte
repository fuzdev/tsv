<script lang="ts">
	// A line comment before a default's `=` stays after the constraint; the
	// `= B` tail continues one indent level in.
	function fn<T extends A // c
= B>(): void {}

	// Default-only parameter: the comment trails the name.
	type C<U // c
= B> = U;
</script>
