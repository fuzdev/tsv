<script lang="ts">
	// with no `from`, a reserved word after the `type` modifier is a local name the spec
	// rejects as an early error, deferred like its unmodified twin
	export { type default };
	export { type if as b };
	export { default as c };
</script>
