<script lang="ts">
	// export→star gap: multiline block hangs the star, indented one level
	export /* x
y */
		* from './a';

	// star→from gap: multiline block hangs the keyword
	export type * /* x
y */
		from './b';

	// star→as gap: multiline block hangs the keyword
	export * /* x
y */
		as ns from './c';
</script>
