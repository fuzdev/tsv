<script lang="ts">
	// Non-null assertion before member access
	aaa.bbb.ccc({
		prop:'value',
	})!.length;

	// Multiple non-null assertions
	aaa!.bbb!.ccc({
		prop:'value',
	})!.length;

	// Non-null on computed member
	aaa.bbb.ccc({
		prop:'value',
	})![0];
</script>
