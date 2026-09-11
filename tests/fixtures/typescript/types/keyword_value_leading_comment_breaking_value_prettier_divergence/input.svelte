<script lang="ts">
	// a type that breaks on its own after a comment the author broke after: the break is
	// forced, so the type hangs one level under the keyword
	type T1 = keyof /* x */
		{
			aaa: 1;
		};

	type T2 = () => /* x */
		{
			aaa: 1;
		};

	type T3 = readonly /* x */
		{
			aaa: 1;
		}[];
</script>
