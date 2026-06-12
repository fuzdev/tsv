<script lang="ts">
	const a = [
		{b: 'c'},
		{d: 'e'},

		// group 1
		{f: 'g'},
		{h: 'i'},

		// group 2

		// group 3
		{j: 'k'},
	];

	function fn1() {
		switch (a) {
			case 'a':
				break;

			// group
			case 'b':
				c++;

			case 'c':

			// between cases
			case 'd':
				d++;

			// trailing 1

			// trailing 2
		}
	}
</script>
