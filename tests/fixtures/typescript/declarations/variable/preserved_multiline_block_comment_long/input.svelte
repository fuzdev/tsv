<script lang="ts">
	// A preserved (non-indentable) multi-line block comment glued to the value: its FIRST line
	// is measured from the `=`, so at 100 columns the value stays on the `=` line
	const a = /* aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	b */ x;
	// 101 columns: the comment hangs under the `=`, its later lines verbatim
	const b =
		/* aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	b */ x;
</script>
