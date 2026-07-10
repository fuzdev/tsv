<script lang="ts">
	// Huggable object member in a function-type union return: the object owns its
	// own expansion — its body indents ONE level and `}` returns to base, with the
	// void member (`| null` / `| void`) trailing the `}`. Mirrors the type-alias RHS.

	// Object + null at exactly 100 chars — stays inline (both us and prettier)
	type Fitxx = () => { propx: BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB } | null;

	// Object + null at 101 chars — hugs `=>`; body at base+1, `}` at base, `| null` trails
	type Break = () => {
		propx: BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB;
	} | null;

	// Void member instead of null — same hug
	type Voidd = () => {
		property_one: string;
		property_two: number;
		property_three: Tttttttttt;
	} | void;

	// Constructor type shares the same return path (`new`)
	type Ctor = new () => {
		property_one: string;
		property_two: number;
		property_three: Tttttt;
	} | null;
</script>
