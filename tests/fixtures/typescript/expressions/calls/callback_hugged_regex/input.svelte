<script lang="ts">
	// Callback with regex trailing arg - should stay hugged
	fn(() => {
		a();
	}, /re/);

	// Callback with longer regex trailing arg
	fn(() => {
		log.aaaaa = x;
	}, /Invalid log aaaaa/);

	// Multiple statements in callback
	fn(() => {
		a();
		b();
	}, /pattern/i);
</script>
