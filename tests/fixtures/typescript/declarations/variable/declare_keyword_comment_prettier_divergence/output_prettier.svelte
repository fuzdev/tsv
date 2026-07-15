<script lang="ts">
	// Block comment between `declare` and the kind keyword — stays where the author
	// wrote it, for every declaration kind
	declare const /* c1 */ a: number;
	declare let /* c2 */ b: string;
	declare var /* c3 */ c: boolean;

	// The same gap under an `export` prefix
	export declare const /* c4 */ d: number;

	// The same gap with multiple declarators
	declare const /* c5 */ e: number, f: string;
</script>
