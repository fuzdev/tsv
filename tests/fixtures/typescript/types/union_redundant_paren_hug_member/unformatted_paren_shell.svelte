<script lang="ts">
	// a redundant paren around a member of a HUGGING union is dropped and changes no
	// layout: the object member still owns its own expansion and the void sibling still
	// trails the `}` - at both slots of the gate, and at every position that asks it

	// a type argument, paren around the object member
	type Arg = G<({
		a: 1;
	}) | null>;

	// a type argument, paren around the void sibling
	type ArgSibling = G<{
		a: 1;
	} | (null)>;

	// `void` is the gate's other void keyword
	type ArgVoid = G<{
		a: 1;
	} | (void)>;

	// a return type, paren around the object member
	function fn(): ({
		a: 1;
	}) | null {}

	// a return type, paren around the void sibling
	function gn(): {
		a: 1;
	} | (void) {}

	// both slots parenthesized at once
	type Both = G<({
		a: 1;
	}) | (null)>;

	// a type alias `=` value
	type Alias = ({
		a: 1;
	}) | null;

	// a variable annotation
	let a: ({
		b: 1;
	}) | null;

	// an interface member
	interface Member {
		a: ({
			b: 1;
		}) | null;
	}

	// a class property
	class Prop {
		a: ({
			b: 1;
		}) | null;
	}

	// an `as` cast - the keyword seam, which hangs when the union does not hug
	const cast = b as ({
		a: 1;
	}) | null;

	// a conditional branch
	type Branch = X extends Y
		? ({
				a: 1;
			}) | null
		: Z;

	// an array element - the redundant paren sits inside the REQUIRED `[]` shell
	type Elem = (({
		a: 1;
	}) | null)[];

	// a type-parameter default
	type Default<
		T = ({
			a: 1;
		}) | null
	> = T;

	// an arrow return type
	const arrow = (): ({
		a: 1;
	}) | null => null;
</script>
