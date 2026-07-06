<script lang="ts">
	// A catch binding may be a contextual type keyword (not a reserved word)
	try {
	} catch (any) {}

	try {
	} catch (string) {}

	// Contextual keyword binding with a type annotation
	try {
	} catch (object: Error) {}
</script>
