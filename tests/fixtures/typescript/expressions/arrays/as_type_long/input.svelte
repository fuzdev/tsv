<script lang="ts">
	function fn() {
		// Array with as type - at 100 chars, stays inline
		const a = ['aaa-aaa', 'bbb-bbb', 'ccc-ccc', 'ddd-ddddd', 'eee-eee'] as Array<TTTTTTTTTTTTTTTTT>;

		// Array with as type - at 101 chars, array expands
		const b = [
			'aaa-aaa',
			'bbb-bbb',
			'ccc-ccc',
			'ddd-ddddd',
			'eee-eee',
		] as Array<TTTTTTTTTTTTTTTTTT>;

		// Array with satisfies - at 101 chars, array expands
		const c = [
			'aaa-aaa',
			'bbb-bbb',
			'ccc-ccc',
			'ddd-ddddd',
			'eee-eee',
		] satisfies Array<TTTTTTTTTTT>;
	}
</script>
