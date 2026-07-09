<script lang="ts">
	interface I {
		// getter with no params, setter with exactly one non-rest param — valid
		get a(): number;
		set a(v: number);
		// the one setter param may be typed or destructuring — valid
		set b([v]: number[]);
	}
</script>
