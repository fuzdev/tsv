<script lang="ts">
	(type) as unknown;
	(module) as X;
	(type) satisfies X;

	// declaration keywords in prettier's Flow dialect keep the pair too
	(component) as X;
	(hook) satisfies X;
</script>
