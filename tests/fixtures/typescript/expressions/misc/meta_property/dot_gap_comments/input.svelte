<script lang="ts">
	// Block comment before the `.` — stays there, hugging the dot
	function f() {
		return new /* c1 */.target;
	}

	// Block comment after the `.` — stays on that side
	function g() {
		return new./* c2 */ target;
	}

	// The same two gaps on `import.meta`
	const a = import /* c3 */.meta.url;
	const b = import./* c4 */ meta.url;
</script>
