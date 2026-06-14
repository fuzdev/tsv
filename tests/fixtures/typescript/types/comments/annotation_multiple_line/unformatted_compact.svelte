<script lang="ts">
	// Union annotation: first comment trails `:`, remaining comments and the type indent one level
	let {a,b}:// union one
	// union two
	A|B=init1;

	// Intersection annotation: first comment trails `:`, remaining comments and the type stay flush
	let {c,d}:// inter one
	// inter two
	A&B=init2;

	// Three line comments: each breaks onto its own line (none merge)
	let {e,f}:// union three a
	// union three b
	// union three c
	A|B=init3;

	// Block comment stays inline after `:`; the line comment ends its line
	let {g,h}:/* lead */// trail
	A|B=init4;
</script>
