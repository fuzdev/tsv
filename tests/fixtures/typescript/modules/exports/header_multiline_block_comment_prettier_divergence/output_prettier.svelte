<script lang="ts">
	// export→star gap: multiline block hangs the star, indented one level
	export * from /* x
y */
	'./a';

	// star→from gap: multiline block hangs the keyword
	export type * from /* x
y */
	'./b';

	// star→as gap: multiline block hangs the keyword
	export * as /* x
y */
	ns from './c';
</script>
