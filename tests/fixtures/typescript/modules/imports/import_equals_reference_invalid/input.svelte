<script lang="ts">
	// An `import X = ...` reference is `require('...')` or an entity name (A.B.C)
	import req = require('y');
	import ent = A.B.C;

	// `import type X = ...` is valid ONLY for an external module reference;
	// an entity-name alias is rejected (see input_invalid_type_modifier).
	import type ext = require('z');
</script>
