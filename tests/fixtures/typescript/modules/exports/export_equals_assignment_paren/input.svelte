<script lang="ts">
	// An assignment as the exported value takes clarity parens, the `export default`
	// twin. A trailing block comment stays before the `;`, outside that pair — this
	// position keeps it there rather than deferring past the terminator.
	export = (x = y) /* c */;
</script>
