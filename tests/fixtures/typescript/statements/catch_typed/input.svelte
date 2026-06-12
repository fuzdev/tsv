<script lang="ts">
	// Typed catch with "unknown"
	try {
	} catch (e: unknown) {}

	// Typed catch with "any"
	try {
	} catch (e: any) {}

	// Typed catch with specific error type
	try {
	} catch (e: Error) {}

	// Catch without binding
	try {
	} catch {}

	// Catch with just binding (no type)
	try {
	} catch (e) {}
</script>
