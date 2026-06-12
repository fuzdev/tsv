<script lang="ts">
	// Short array - stays inline with comment
	fn(['element']/* after */);

	// Long array - expands with comment preserved
	fn(
		[
			'element1_long_string_here_aaaaaaaaaaaaaaaa',
			'element2_another_long_string_bbbbbbbbbbbb',
		]/* after */,
	);
</script>
