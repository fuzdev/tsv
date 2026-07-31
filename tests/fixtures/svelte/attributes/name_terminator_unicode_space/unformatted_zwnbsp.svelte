<!--
	Svelte ends an attribute or directive name run at any JS `\s` character, so a
	Unicode space there separates two names (see the unformatted_* variants).
	U+0085 NEL is not in JS `\s`, so it stays part of one name.
-->
<script>
	let a = 1;
</script>

<div data-attr﻿data-attr2></div>
<div class:a﻿data-attr></div>
<div data-ab></div>
