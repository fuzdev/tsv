<script module>
	function helper() {}
	/**
	 * Shared across instances.
	 */
	export const shared = 1;
</script>

<script>
	/**
	 * The count shown below.
	 * @type {number}
	 */
	let count = $state(shared);
	class Tally {
		/**
		 * Bump it.
		 */
		bump() {
			/*
			 * inner note
			 */
			count += 1;
		}
	}
	const tally = new Tally();
	helper();
</script>

<button onclick={() => tally.bump()}>{count}</button>
