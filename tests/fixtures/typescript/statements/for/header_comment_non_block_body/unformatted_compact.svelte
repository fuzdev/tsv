<script lang="ts">
	// own-line comment forces the header multiline; non-block body goes to its own line
	for(let i=0;
	// c
	i<10;i++)fn();

	// trailing line comment in the header also forces it multiline
	for(let i=0; // init
	i<10;i++)fn();

	// multiline header + block comment between ) and body — both drop to the body line
	for(let i=0;
	// c
	i<10;i++) /* b */ fn();
</script>
