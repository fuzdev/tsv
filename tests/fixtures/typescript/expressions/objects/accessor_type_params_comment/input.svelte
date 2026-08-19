<script lang="ts">
	// Comments around an accessor's type parameters are preserved once (not duplicated onto the key)
	const o = {
		get a/* c1 */ <T>() {
			return 1;
		},
		get b</* c2 */ T>() {
			return 1;
		},
		set c<T /* c3 */>(v: T) {},
		set d<T> /* c4 */(v: T) {}
	};
</script>
