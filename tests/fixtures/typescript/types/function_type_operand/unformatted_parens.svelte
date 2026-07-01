<script lang="ts">
	// A function type is not grammatically allowed as a bare union/intersection
	// constituent or type-operator operand — it must be parenthesized there
	// (the unparenthesized forms are rejected; see input_invalid_*).
	type A1 = ((B) & ((() => C)));
	type A2 = (B | ((([a]) => C)) | (D));
	type A3 = (keyof ((() => C)));

	// Redundant parens around a type-operator operand are stripped
	// (`readonly ([a])[]` normalizes to this).
	type A4 = (B & (readonly ([a])[]));
</script>
