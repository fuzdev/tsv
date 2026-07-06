<script lang="ts">
	// `type` then `as <name>` (one `as`) is a VALUE import of a binding named
	// `type`, renamed — NOT the type-only modifier.
	import { type as age } from 'a';

	// `type as` alone (no trailing name) is a TYPE-only import of a binding
	// named `as`.
	import { type as } from 'b';

	// Two `as` then a name: `type` is the modifier, `as` the imported name,
	// renamed — a TYPE import.
	import { type as as year } from 'c';

	// `type <name>` is the type-only modifier.
	import { type Foo } from 'd';

	// Bare `type` is a value import of a binding named `type`.
	import { type } from 'e';

	// A type-only modifier on a later specifier (not the first).
	import { g, type H } from 'f';
</script>
