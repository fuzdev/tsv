<script lang="ts">
	// An object-literal union return (`{ … } | null` / `| void`) hugs the return `:`
	// block-style across declaration contexts — the object owns its own expansion and
	// the void member trails the `}`. Mirrors the type-alias RHS; complements
	// return_type_generic_union_long (a TSTypeReference member, which does not hug).

	// Function declaration at exactly 100 chars — stays inline (both us and prettier)
	function fitxx(): { propx: Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb } | null {}

	// Function declaration at 101 chars — hugs `:`; body at base+1, `}` at base
	function brkxx(): {
		propx: Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;
	} | null {}

	// Class method — same hug, with a `| void` member
	class C {
		method(): {
			property_one: string;
			property_two: number;
			property_three: Ttttttttttttt;
		} | void {}
	}

	// Arrow — the return type hugs before `=>`
	const arrow = (): {
		property_one: string;
		property_two: number;
		property_three: Ttttttt;
	} | null => {};
</script>
