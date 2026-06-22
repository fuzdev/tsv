<script lang="ts">
	// Optional chain ending with member - object hugs inline
	aaa?.bbb?.ccc({
		prop: 'value'
	})?.length;

	// Longer optional chain
	aaaaaaaa?.bbbbbbbb?.cccccccc({
		prop: 'value'
	})?.length;

	// Mixed optional and regular
	aaa.bbb?.ccc({
		prop: 'value'
	})?.length;
</script>
