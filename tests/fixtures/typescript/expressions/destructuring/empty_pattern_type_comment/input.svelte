<script lang="ts">
	// A comment in an empty destructuring pattern's type annotation belongs to
	// the type, not the empty {} / [].
	const {}: /* c */ T = x;
	const []: /* c */ U = y;
</script>
