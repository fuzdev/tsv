<script lang="ts">
	// keyof as an array element keeps its parens ((keyof T)[] != keyof T[])
	type A = (keyof T)[];

	// readonly array as an array element keeps its parens
	type B = (readonly string[])[];

	// keyof as an indexed-access object keeps its parens (contrast)
	type C = (keyof T)['x'];
</script>
