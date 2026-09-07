<script lang="ts">
	// a redundant paren around a member of a HUGGING union is dropped and changes no
	// layout: the object member still owns its own expansion and the void sibling still
	// trails the `}` - at both slots of the gate and both positions that ask it

	// a type argument, paren around the object member
	type Arg = G<({
		a: 1
	}) | null>;

	// a type argument, paren around the void sibling
	type ArgSibling = G<{
		a: 1
	} | (null)>;

	// `void` is the gate's other void keyword
	type ArgVoid = G<{
		a: 1
	} | (void)>;

	// a return type, paren around the object member
	function fn(): ({
		a: 1
	}) | null {}

	// a return type, paren around the void sibling
	function gn(): {
		a: 1
	} | (void) {}

	// both slots parenthesized at once
	type Both = G<({
		a: 1
	}) | (null)>;
</script>
