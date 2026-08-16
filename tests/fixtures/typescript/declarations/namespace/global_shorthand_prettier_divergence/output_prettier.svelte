<script lang="ts">
	// A bodyless `declare global` is the ambient-module production's other name
	declare;
	global;

	// A block comment written before the `;` stays before it
	declare;
	global; /* c */

	// The bodied form is unchanged
	declare global {
		var a: string;
	}

	// Without `declare`, a bodyless `global` is an ordinary expression statement
	global;

	// And so is one inside an ambient module, where `declare` is implicit
	declare module 'a' {
		global;
	}
</script>
