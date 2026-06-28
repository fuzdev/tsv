<script lang="ts">
	// Union type: comment stays before, type indented one level
	interface B {
		b:
			// union
			X | Y;
	}
</script>
