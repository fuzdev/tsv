<script lang="ts">
	export import Types = Namespace.Types;
	export import Inner = Outer.Inner.Deep;
	export import Ext = require('mod');

	// `export import type X = require(...)` is valid (external module reference);
	// the entity-name form is rejected — see input_invalid_export_import_type.
	export import type ExtType = require('typed');
</script>
