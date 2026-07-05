<script lang="ts">
	// Contextual type keywords are valid type-alias names (they are not reserved words)
	type any = boolean;
	type string = number;
	type object = unknown;

	// declare / export type-alias variants
	declare type unknown = never;
	export type symbol = bigint;
</script>
