<script lang="ts">
	// A block hugging the next param leads it; the line comment forces multiline
	type A = (
		a: string, // t
		b: number, /* c */ c: boolean
	) => void;

	// Before-comma and after-comma blocks in one gap each keep their side
	type B = (
		a: string, // t
		b: number /* c1 */, /* c2 */ c: boolean
	) => void;

	// Constructor type: same rule
	type C = new (
		a: string, // t
		b: number, /* c */ c: boolean
	) => Type;
</script>
