<script lang="ts">
	// `<<` splits into a type assertion whose asserted type is a generic
	// function type — prettier's typescript parser cannot split it here
	// (see prettier_rejects.txt)
	const a = <<T>() => R>x;
</script>
