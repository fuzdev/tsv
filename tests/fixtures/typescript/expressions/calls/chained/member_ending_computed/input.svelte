<script lang="ts">
	// Computed member ending with string key
	aaa.bbb.ccc({
		prop: 'value',
	})['key'];

	// Computed member ending with number
	aaa.bbb.ccc({
		prop: 'value',
	})[0];

	// Computed member ending with variable
	aaa.bbb.ccc({
		prop: 'value',
	})[index];
</script>
