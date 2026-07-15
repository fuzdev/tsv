<script lang="ts">
	// Block comment between `declare` and the kind keyword — stays where the author
	// wrote it, for every declaration kind
	declare /* c1 */ const a: number;
	declare /* c2 */ let b: string;
	declare /* c3 */ var c: boolean;

	// The same gap under an `export` prefix
	export declare /* c4 */ const d: number;

	// The same gap with multiple declarators
	declare /* c5 */ const e: number, f: string;
</script>
