<script lang="ts">
	// Block comment between `export` and `default` on a decorated class expression —
	// the decorators print on their own line, the comment stays after `export`
	export default /* c */
	@dec
	class {}
</script>
